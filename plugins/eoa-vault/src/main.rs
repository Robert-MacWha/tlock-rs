//! EOA Vault Plugin
//!
//! This is a simple exemplar vault plugin that manages an Externally Owned
//! Account (EOA) using a private key provided by the user. It supports
//! operations for native ETH and a predefined set of ERC20 tokens.

use std::{collections::HashMap, io::stderr};

use alloy::{
    hex,
    network::TransactionBuilder,
    primitives::{Address, U256, address},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
};
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    runner::PluginRunner,
    state::{get_state, set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId, AssetType, ChainId},
        component::{button_input, container, form, heading, submit_input, text, text_input},
        domains::Domain,
        entities::{EntityId, EthProviderId, PageId, VaultId},
        eth::{self},
        global, host, page, plugin, vault,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, RpcErrorContext, ToRpcResult},
        transport::Transport,
    },
};
use tracing::{info, warn};
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize, Default, Debug)]
struct PluginState {
    vaults: HashMap<EntityId, Vault>,
    provider_id: EthProviderId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Vault {
    private_key: String,
    address: Address,
}

//? We can use alloy-generated bindings for creating contract interfaces,
//? making on-chain calls much easier.
sol! {
    #[sol(rpc)]
    contract ERC20 {
        function balanceOf(address owner) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

const CHAIN_ID: u64 = 11155111; // Sepolia
const ERC20S: [Address; 2] = [
    address!("0x1c7d4b196cb0c7b01d743fbc6116a902379c7238"), // USDC
    address!("0xfff9976782d46cc05630d1f6ebab18b2324d6b14"), // WETH
];

// ---------- Plugin Handlers ----------

async fn init(transport: Transport, _params: ()) -> Result<(), RpcError> {
    info!("Calling Init on Vault Plugin");

    // ? Register the vault's page
    let provider_id = host::RequestEthProvider
        .call_async(transport.clone(), ChainId::Evm(Some(CHAIN_ID)))
        .await?;
    let state = PluginState {
        vaults: HashMap::new(),
        provider_id,
    };
    set_state(transport.clone(), &state)?;

    host::RegisterEntity
        .call_async(transport.clone(), Domain::Page)
        .await?;

    Ok(())
}

async fn ping(transport: Transport, _params: ()) -> Result<String, RpcError> {
    let state: PluginState = try_get_state(transport.clone())?;

    let chain_id = eth::ChainId
        .call_async(transport, state.provider_id)
        .await?;
    Ok(format!("Pong! Connected to chain: {}", chain_id))
}

// ---------- Vault Handlers ----------

async fn get_assets(
    transport: Transport,
    params: VaultId,
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let vault_id = params;
    info!("Received get_assets request for vault: {}", vault_id);

    let vault = get_vault(transport.clone(), vault_id)?;

    let state: PluginState = try_get_state(transport.clone())?;
    let provider = ProviderBuilder::new()
        .connect_client(AlloyBridge::new(transport.clone(), state.provider_id));

    // Fetch native ETH balance
    let balance = provider.get_balance(vault.address).await.rpc_err()?;

    info!("ETH balance for vault {}: {}", vault_id, balance);
    let mut balances = vec![(AssetId::eth(CHAIN_ID), balance)];

    // Fetch ERC20 balances
    //? We could choose to filter out zero balances here if desired.
    for &erc20_address in ERC20S.iter() {
        let contract = ERC20::new(erc20_address, &provider);
        let balance = contract.balanceOf(vault.address).call().await.rpc_err()?;

        info!(
            "ERC20 balance for vault {}, token {}: {}",
            vault_id, erc20_address, balance
        );
        balances.push((AssetId::erc20(CHAIN_ID, erc20_address), balance));
    }

    Ok(balances)
}

async fn get_deposit_address(
    transport: Transport,
    params: (VaultId, AssetId),
) -> Result<AccountId, RpcError> {
    let (vault_id, asset_id) = params;
    info!("Received GetDepositAddress request for vault: {}", vault_id);

    validate_chain_id(asset_id.chain_id())?;

    let vault = get_vault(transport.clone(), vault_id)?;
    let account_id = AccountId::new_evm(CHAIN_ID, vault.address);

    // If the asset is supported, we MUST return a valid address.
    match &asset_id.asset {
        AssetType::Slip44(60) => Ok(account_id),
        AssetType::Erc20(addr) if ERC20S.contains(addr) => Ok(account_id),
        _ => Err(RpcError::Custom(
            "Unsupported asset for deposit address".into(),
        )),
    }
}

async fn withdraw(
    transport: Transport,
    params: (VaultId, AccountId, AssetId, U256),
) -> Result<(), RpcError> {
    let (vault_id, to_address, asset_id, amount) = params;
    info!(
        "Received Withdraw request for vault: {}, to address: {}, asset: {}, amount: {}",
        vault_id, to_address, asset_id, amount
    );

    validate_chain_id(asset_id.chain_id())?;
    validate_chain_id(to_address.chain_id())?;

    let to_addr = to_address
        .as_evm_address()
        .ok_or_else(|| RpcError::Custom("Invalid to address".into()))?;

    let vault = get_vault(transport.clone(), vault_id)?;
    let signer: PrivateKeySigner = vault.private_key.parse().rpc_err()?;
    let state: PluginState = try_get_state(transport.clone())?;
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_client(AlloyBridge::new(transport.clone(), state.provider_id));

    match &asset_id.asset {
        AssetType::Slip44(60) => withdraw_eth(&provider, to_addr, amount).await,
        AssetType::Erc20(token) => withdraw_erc20(&provider, *token, to_addr, amount).await,
        _ => Err(RpcError::Custom(
            "Unsupported asset type for withdrawal".into(),
        )),
    }
}

async fn withdraw_eth(provider: impl Provider, to: Address, amount: U256) -> Result<(), RpcError> {
    let tx = TransactionRequest::default().to(to).with_value(amount);
    let tx_hash = provider
        .send_transaction(tx)
        .await
        .rpc_err()?
        .watch()
        .await
        .rpc_err()?;
    info!("ETH withdrawal transaction sent with hash: {}", tx_hash);
    Ok(())
}

async fn withdraw_erc20(
    provider: impl Provider,
    token_address: Address,
    to: Address,
    amount: U256,
) -> Result<(), RpcError> {
    if !ERC20S.contains(&token_address) {
        return Err(RpcError::Custom(
            "Unsupported ERC20 token for withdrawal".into(),
        ));
    }
    let contract = ERC20::new(token_address, &provider);
    let tx_hash = contract
        .transfer(to, amount)
        .send()
        .await
        .rpc_err()?
        .watch()
        .await
        .rpc_err()?;
    info!("ERC20 withdrawal transaction sent with hash: {}", tx_hash);
    Ok(())
}

// ---------- UI Handlers ----------

async fn on_load(transport: Transport, page_id: PageId) -> Result<(), RpcError> {
    info!("OnPageLoad called for page: {}", page_id);

    let component = container(vec![
        heading("EOA Vault"),
        text("This is an example vault plugin. Please enter a dev private key."),
        form(
            "private_key_form",
            vec![
                text_input("dev_private_key", "Enter your private key"),
                submit_input("Update"),
            ],
        ),
        button_input("generate_dev_key", "Generate Dev Key"),
    ]);

    host::SetPage
        .call_async(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Transport,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated in Vault Plugin: {:?}", event);

    let private_key_hex = match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "generate_dev_key" => {
            let signer = PrivateKeySigner::random();
            let private_key = signer.to_bytes();
            hex::encode(private_key)
        }
        page::PageEvent::FormSubmitted(_, form_data) => form_data
            .get("dev_private_key")
            .context("No private key in form")?
            .clone(),
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    };

    let signer: PrivateKeySigner = private_key_hex.parse().context("Invalid private key")?;
    let address = signer.address();

    let entity_id = host::RegisterEntity
        .call_async(transport.clone(), Domain::Vault)
        .await?;

    let mut state: PluginState = get_state(transport.clone());
    state.vaults.insert(
        entity_id,
        Vault {
            private_key: private_key_hex.clone(),
            address,
        },
    );
    set_state(transport.clone(), &state)?;

    let component = container(vec![
        heading("EOA Vault"),
        text(&format!("Address: {}", address)),
        text(&format!("Private Key: {}", private_key_hex)),
    ]);
    host::SetPage
        .call_async(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

// ---------- Helpers ----------
fn validate_chain_id(chain_id: &ChainId) -> Result<(), RpcError> {
    match chain_id {
        ChainId::Evm(Some(id)) if *id == CHAIN_ID => Ok(()),
        ChainId::Evm(Some(id)) => Err(RpcError::Custom(format!("Unsupported EVM chain: {}", id))),
        _ => Err(RpcError::Custom("Unsupported chain ID".to_string())),
    }
}

fn get_vault(transport: Transport, id: VaultId) -> Result<Vault, RpcError> {
    let state: PluginState = get_state(transport.clone());
    let vault = state.vaults.get(&id.into()).ok_or_else(|| {
        warn!("vaults: {:?}", state.vaults.keys());
        RpcError::Custom(format!("Vault ID not found: {}", id))
    })?;

    Ok(vault.clone())
}

/// Plugin entrypoint where the host initiates communication.
///
/// # Lifecycle
///
/// Each plugin request runs in an isolated WASM runtime:
/// 1. Host spawns new WASM instance and calls main()
/// 2. Host immediately sends one JSON-RPC request via stdin
/// 3. Plugin processes request, may make JSON-RPC calls to host (via stdout)
/// 4. Host responds to plugin requests via stdin
/// 5. Plugin writes final response to stdout and terminates
///
/// Multiple concurrent requests run in separate isolated runtimes.
///
/// # Execution Model
///
/// - **Stateless**: Each runtime is fresh; persist data via host calls
/// - **I/O**: No direct file/network access - all I/O through host calls
/// - **Communication**: Bidirectional JSON-RPC over stdin/stdout
/// - **Async**: Full async support via wasm32-wasip1 syscalls (tokio, etc.)
///
/// # Initialization
///
/// On first plugin load, the host calls `plugin::Init` for setup.
/// Subsequent requests skip init and directly invoke registered methods.
fn main() {
    // Setup logging - host captures stderr and forwards to its logging system
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting plugin...");

    // Register method handlers and start processing.
    // The server automatically:
    // - Creates stdin/stdout JSON-RPC transport
    // - Sets up async runtime
    // - Reads initial host request and routes to handler
    // - Handles bidirectional RPC until final response
    PluginRunner::new()
        .with_method(plugin::Init, init)
        .with_method(global::Ping, ping)
        .with_method(vault::GetAssets, get_assets)
        .with_method(vault::Withdraw, withdraw)
        .with_method(vault::GetDepositAddress, get_deposit_address)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .run();
}
