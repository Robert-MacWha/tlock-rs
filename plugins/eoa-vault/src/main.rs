//! EOA Vault Plugin
//!
//! This is a simple exemplar vault plugin that manages an Externally Owned
//! Account (EOA) using a private key provided by the user. It supports
//! operations for native ETH and a predefined set of ERC20 tokens.

use std::{collections::HashMap, io::stderr};

use alloy::{
    network::TransactionBuilder,
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
};
use erc20s::{ERC20S, get_erc20_by_address};
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    runner::PluginRunner,
    state::{get_state, set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId, AssetType, ChainId},
        component::{
            Component, account, asset, button_input, container, form, heading, heading2, hex,
            submit_input, text, text_input, unordered_list,
        },
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
    vault: Option<Vault>,
    provider_id: EthProviderId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Vault {
    entity_id: EntityId,
    private_key: FixedBytes<32>,
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

// ---------- Plugin Handlers ----------

async fn init(transport: Transport, _params: ()) -> Result<(), RpcError> {
    info!("Calling Init on Vault Plugin");

    // ? Register the vault's page
    let provider_id = host::RequestEthProvider
        .call_async(transport.clone(), ChainId::Evm(Some(CHAIN_ID)))
        .await?;
    let state = PluginState {
        vault: None,
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
    let assets = get_vault_assets(transport.clone(), &vault).await?;
    Ok(assets)
}

async fn get_vault_assets(
    transport: Transport,
    vault: &Vault,
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let state: PluginState = try_get_state(transport.clone())?;
    let provider = ProviderBuilder::new()
        .connect_client(AlloyBridge::new(transport.clone(), state.provider_id));

    // Fetch native ETH balance
    let balance = provider.get_balance(vault.address).await.rpc_err()?;

    let mut balances = vec![(AssetId::eth(CHAIN_ID), balance)];

    // Fetch ERC20 balances
    //? We could choose to filter out zero balances here if desired.
    let mut erc20_futures = Vec::new();
    for erc20 in &ERC20S {
        let address = erc20.address;
        let contract = ERC20::new(address, &provider);
        erc20_futures.push(async move {
            let balance = contract.balanceOf(vault.address).call().await.rpc_err()?;
            Ok::<_, RpcError>((AssetId::erc20(CHAIN_ID, address), balance))
        });
    }

    let erc20_balances = futures::future::try_join_all(erc20_futures).await?;
    balances.extend(erc20_balances);

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
        AssetType::Erc20(addr) if get_erc20_by_address(addr).is_some() => Ok(account_id),
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
    let signer: PrivateKeySigner =
        PrivateKeySigner::from_bytes(&vault.private_key).context("Invalid private key")?;
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
    if get_erc20_by_address(&token_address).is_none() {
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

    let state: PluginState = get_state(transport.clone());
    let component = build_ui(transport.clone(), &state).await;
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

    match event {
        page::PageEvent::ButtonClicked(id) if id == "generate_dev_key" => {
            let signer = PrivateKeySigner::random();
            handle_new_signer(transport.clone(), signer.clone()).await?;
        }
        page::PageEvent::ButtonClicked(id) if id == "refresh_assets" => {
            // Simply rebuild the UI to refresh asset balances
        }
        page::PageEvent::FormSubmitted(id, form_data) if id == "private_key_form" => {
            handle_dev_private_key(transport.clone(), form_data).await?;
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
        }
    }

    let state: PluginState = get_state(transport.clone());
    let component = build_ui(transport.clone(), &state).await;
    host::SetPage
        .call_async(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn handle_dev_private_key(
    transport: Transport,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let private_key_hex = form_data
        .get("dev_private_key")
        .context("Private key not in form data")?;

    let private_key_hex = private_key_hex.trim().trim_start_matches("0x").to_string();
    let signer: PrivateKeySigner = private_key_hex.parse().context("Invalid private key")?;
    handle_new_signer(transport.clone(), signer).await
}

async fn handle_new_signer(transport: Transport, signer: PrivateKeySigner) -> Result<(), RpcError> {
    let address = signer.address();

    let entity_id = host::RegisterEntity
        .call_async(transport.clone(), Domain::Vault)
        .await?;

    let mut state: PluginState = get_state(transport.clone());
    state.vault = Some(Vault {
        entity_id,
        private_key: signer.to_bytes(),
        address,
    });
    set_state(transport.clone(), &state)?;

    Ok(())
}

async fn build_ui(transport: Transport, state: &PluginState) -> Component {
    let mut sections = vec![
        heading("EOA Vault"),
        text("Example vault plugin managing an externally owned account using a private key"),
    ];

    let Some(vault) = &state.vault else {
        sections.push(heading2("Create EOA"));
        sections.push(form(
            "private_key_form",
            vec![
                text_input("dev_private_key", "Private Key", "0xabc123"),
                submit_input("Create Vault"),
            ],
        ));
        sections.push(button_input("generate_dev_key", "Random Vault"));
        return container(sections);
    };

    sections.push(heading2("Vault Info"));
    sections.push(text("Vault Address:"));
    sections.push(account(AccountId::new_evm(CHAIN_ID, vault.address)));
    sections.push(text("Private Key:"));
    sections.push(hex(vault.private_key.as_slice()));

    let Ok(balances) = get_vault_assets(transport.clone(), &vault).await else {
        sections.push(text("Error fetching assets"));
        return container(sections);
    };

    sections.push(heading2("Assets"));
    let balances = balances
        .into_iter()
        .map(|(id, bal)| (id.to_string(), asset(id.clone(), Some(bal.clone()))));
    sections.push(unordered_list(balances));
    sections.push(button_input("refresh_assets", "Refresh"));

    return container(sections);
}

// ---------- Helpers ----------
fn validate_chain_id(chain_id: &ChainId) -> Result<(), RpcError> {
    match chain_id {
        ChainId::Evm(Some(id)) if *id == CHAIN_ID => Ok(()),
        ChainId::Evm(Some(id)) => Err(RpcError::Custom(format!("Unsupported EVM chain: {}", id))),
        _ => Err(RpcError::Custom("Unsupported chain ID".to_string())),
    }
}

fn get_vault(transport: Transport, _id: VaultId) -> Result<Vault, RpcError> {
    let state: PluginState = get_state(transport.clone());
    let vault = state
        .vault
        .clone()
        .ok_or_else(|| RpcError::Custom("No vault configured in plugin state".to_string()))?;

    Ok(vault)
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
