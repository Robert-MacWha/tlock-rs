//! EOA Vault Plugin
//!
//! This is a simple exemplar vault plugin that manages an Externally Owned
//! Account (EOA) using a private key provided by the user. It supports
//! operations for native ETH and a predefined set of ERC20 tokens.

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
use std::{collections::HashMap, io::stderr, sync::Arc};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    server::PluginServer,
    state::{get_state, set_state},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId, AssetType, ChainId},
        component::{
            Component, button_input, container, form, heading, submit_input, text, text_input,
        },
        domains::Domain,
        entities::{EntityId, EthProviderId, PageId, VaultId},
        eth::{self},
        global, host, page, plugin, vault,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, to_rpc_err},
        transport::JsonRpcTransport,
    },
};
use tracing::{info, warn};
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize, Default, Debug)]
struct PluginState {
    vaults: HashMap<EntityId, Vault>,
    page_id: Option<PageId>,
    eth_provider_id: Option<EthProviderId>,
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

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    info!("Calling Init on Vault Plugin");

    // ? Register the vault's page
    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    request_eth_provider(transport.clone()).await?;

    Ok(())
}

async fn ping(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<String, RpcError> {
    let provider_id = request_eth_provider(transport.clone()).await?;
    info!(
        "Pong received, querying chain ID from Eth provider: {}",
        provider_id
    );

    let chain_id = eth::ChainId.call(transport, provider_id).await?;
    Ok(format!("Pong! Connected to chain: {}", chain_id))
}

// ---------- Vault Handlers ----------

async fn get_assets(
    transport: Arc<JsonRpcTransport>,
    params: VaultId,
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let vault_id = params;
    info!("Received get_assets request for vault: {}", vault_id);

    let vault = get_vault(&transport, vault_id).await?;

    let provider_id = request_eth_provider(transport.clone()).await?;
    let provider =
        ProviderBuilder::new().connect_client(AlloyBridge::new(transport.clone(), provider_id));

    // Fetch native ETH balance
    let balance = provider
        .get_balance(vault.address)
        .await
        .map_err(to_rpc_err)?;

    info!("ETH balance for vault {}: {}", vault_id, balance);
    let mut balances = vec![(AssetId::eth(CHAIN_ID), balance)];

    // Fetch ERC20 balances
    //? We could choose to filter out zero balances here if desired.
    for &erc20_address in ERC20S.iter() {
        let contract = ERC20::new(erc20_address, &provider);
        let balance = contract
            .balanceOf(vault.address)
            .call()
            .await
            .map_err(to_rpc_err)?;

        info!(
            "ERC20 balance for vault {}, token {}: {}",
            vault_id, erc20_address, balance
        );
        balances.push((AssetId::erc20(CHAIN_ID, erc20_address), balance));
    }

    Ok(balances)
}

async fn get_deposit_address(
    transport: Arc<JsonRpcTransport>,
    params: (VaultId, AssetId),
) -> Result<AccountId, RpcError> {
    let (vault_id, asset_id) = params;
    info!("Received GetDepositAddress request for vault: {}", vault_id);

    validate_chain_id(asset_id.chain_id())?;

    let vault = get_vault(&transport, vault_id).await?;
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

async fn on_deposit(
    _transport: Arc<JsonRpcTransport>,
    params: (VaultId, AssetId),
) -> Result<(), RpcError> {
    let (vault_id, asset_id) = params;
    info!(
        "Received OnDeposit notification for vault: {}, asset: {}",
        vault_id, asset_id
    );

    // Since this is an EOA vault, no action is needed on deposit. Other
    // vaults might log here, forward funds to another address, call some
    // api, etc.

    Ok(())
}

async fn withdraw(
    transport: Arc<JsonRpcTransport>,
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

    let vault = get_vault(&transport, vault_id).await?;
    let signer: PrivateKeySigner = vault.private_key.parse().map_err(to_rpc_err)?;
    let provider_id = request_eth_provider(transport.clone()).await?;
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_client(AlloyBridge::new(transport.clone(), provider_id));

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
        .map_err(to_rpc_err)?
        .watch()
        .await
        .map_err(to_rpc_err)?;
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
        .map_err(to_rpc_err)?
        .watch()
        .await
        .map_err(to_rpc_err)?;
    info!("ERC20 withdrawal transaction sent with hash: {}", tx_hash);
    Ok(())
}

async fn request_eth_provider(transport: Arc<JsonRpcTransport>) -> Result<EthProviderId, RpcError> {
    let mut state: PluginState = get_state(transport.clone()).await;
    if let Some(provider_id) = state.eth_provider_id {
        return Ok(provider_id);
    }

    let chain_id: ChainId = ChainId::new_evm(CHAIN_ID);
    let provider_id = host::RequestEthProvider
        .call(transport.clone(), chain_id)
        .await?
        .ok_or_else(|| RpcError::Custom("Failed to obtain Eth Provider".into()))?;

    state.eth_provider_id = Some(provider_id);
    set_state(transport.clone(), &state).await?;

    Ok(provider_id)
}

// ---------- UI Handlers ----------

async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    info!("OnPageLoad called for page: {}", page_id);

    let mut state: PluginState = get_state(transport.clone()).await;
    state.page_id = Some(page_id);
    set_state(transport.clone(), &state).await?;

    let component = private_key_form_component("Enter your private key");
    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated in Vault Plugin: {:?}", event);

    let private_key_hex;
    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "generate_dev_key" => {
            let signer = PrivateKeySigner::random();
            let private_key = signer.to_bytes();
            private_key_hex = hex::encode(private_key);
        }
        page::PageEvent::FormSubmitted(_, form_data) => {
            let Some(pk) = form_data.get("dev_private_key") else {
                return Err(RpcError::Custom("Private key not found in form".into()));
            };
            private_key_hex = pk.clone();
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    }

    let signer: PrivateKeySigner = private_key_hex
        .parse()
        .map_err(|_| RpcError::Custom("Invalid private key".into()))?;
    let address = signer.address();

    let entity_id = host::RegisterEntity
        .call(transport.clone(), Domain::Vault)
        .await?;

    let mut state: PluginState = get_state(transport.clone()).await;
    state.vaults.insert(
        entity_id,
        Vault {
            private_key: private_key_hex.clone(),
            address,
        },
    );
    set_state(transport.clone(), &state).await?;

    let component = text(&format!(
        "Vault created!\n\nAddress: {}\n\nPrivate Key: {}",
        address, private_key_hex
    ));
    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

fn private_key_form_component(preview: &str) -> Component {
    container(vec![
        heading("EOA Vault"),
        text("This is an example vault plugin. Please enter a dev private key."),
        button_input("generate_dev_key", "Generate Dev Key"),
        form(
            "private_key_form",
            vec![
                text_input("dev_private_key", preview),
                submit_input("Update"),
            ],
        ),
    ])
}

// ---------- Helpers ----------
fn validate_chain_id(chain_id: &ChainId) -> Result<(), RpcError> {
    match chain_id {
        ChainId::Evm(Some(id)) if *id == CHAIN_ID => Ok(()),
        ChainId::Evm(Some(id)) => Err(RpcError::Custom(format!("Unsupported EVM chain: {}", id))),
        _ => Err(RpcError::Custom("Unsupported chain ID".to_string())),
    }
}

async fn get_vault(transport: &Arc<JsonRpcTransport>, id: VaultId) -> Result<Vault, RpcError> {
    let state: PluginState = get_state(transport.clone()).await;
    let vault = state
        .vaults
        .get(&id.into())
        .ok_or_else(|| RpcError::Custom(format!("Vault ID not found: {}", id).into()))?;

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
    PluginServer::new_with_transport()
        .with_method(plugin::Init, init)
        .with_method(global::Ping, ping)
        .with_method(vault::GetAssets, get_assets)
        .with_method(vault::Withdraw, withdraw)
        .with_method(vault::GetDepositAddress, get_deposit_address)
        .with_method(vault::OnDeposit, on_deposit)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .run();
}
