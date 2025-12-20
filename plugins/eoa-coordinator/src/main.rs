//! Basic EOA Coordinator Plugin implementation.
//!
//! NOT DESIGNED FOR PRODUCTION USE.
//!
//! This is a minimal, very insecure implementation of a Coordinator Plugin. It
//! stores its private key in memory and in plaintext host storage, does not
//! authenticate requests, and does not do any validation of incoming data. It
//! is intended purely for demonstration and testing.
use std::{io::stderr, sync::Arc};

use alloy::{
    hex,
    primitives::{Address, FixedBytes},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
};
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    server::PluginServer,
    state::{set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        alloy::primitives::U256,
        caip::{AccountId, AssetId, AssetType, ChainId},
        component::{button_input, container, form, heading, submit_input, text, text_input},
        coordinator,
        domains::Domain,
        entities::{CoordinatorId, EntityId, EthProviderId, PageId, VaultId},
        global, host,
        page::{self, PageEvent},
        plugin, vault,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, to_rpc_err},
        transport::JsonRpcTransport,
    },
};
use tracing::{info, trace};
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize, Debug)]
struct State {
    vault_id: VaultId,
    provider_id: EthProviderId,
    coordinator_id: Option<EntityId>,
    private_key: Option<FixedBytes<32>>,
    account: Option<AccountId>,
}

sol! {
    #[sol(rpc)]
    contract ERC20 {
        function balanceOf(address owner) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

const CHAIN_ID: u64 = 11155111; // Sepolia

#[derive(Debug)]
struct ReturnAsset {
    asset: EvmAsset,
    deposit_address: Address,
}

#[derive(Debug)]
enum EvmAsset {
    Eth,
    Erc20(Address),
}

async fn ping(transport: Arc<JsonRpcTransport>, _: ()) -> Result<String, RpcError> {
    global::Ping.call(transport, ()).await?;
    Ok("pong".to_string())
}

async fn init(transport: Arc<JsonRpcTransport>, _: ()) -> Result<(), RpcError> {
    let vault_id = host::RequestVault.call(transport.clone(), ()).await?;
    info!("Obtained vault ID: {}", vault_id);

    let provider_id = host::RequestEthProvider
        .call(transport.clone(), ChainId::new_evm(CHAIN_ID))
        .await?;

    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    let state = State {
        vault_id,
        provider_id,
        coordinator_id: None,
        private_key: None,
        account: None,
    };

    set_state(transport.clone(), &state).await?;
    Ok(())
}

async fn get_session(
    transport: Arc<JsonRpcTransport>,
    params: (CoordinatorId, ChainId, Option<AccountId>),
) -> Result<AccountId, RpcError> {
    let state: State = try_get_state(transport.clone()).await?;
    let (coordinator_id, chain_id, maybe_account_id) = params;

    if Some(coordinator_id.into()) != state.coordinator_id {
        return Err(RpcError::Custom("Invalid CoordinatorId".into()));
    }

    // TODO: Support arbitrary evm chain IDs
    if chain_id != ChainId::new_evm(CHAIN_ID) {
        return Err(RpcError::Custom("Invalid ChainId".into()));
    }

    let Some(state_account_id) = state.account else {
        return Err(RpcError::Custom("No Account configured".into()));
    };

    if let Some(account_id) = maybe_account_id
        && account_id != state_account_id
    {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    Ok(state_account_id)
}

async fn get_assets(
    transport: Arc<JsonRpcTransport>,
    params: (CoordinatorId, AccountId),
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let state: State = try_get_state(transport.clone()).await?;
    let (coordinator_id, account_id) = params;

    if Some(coordinator_id.into()) != state.coordinator_id {
        return Err(RpcError::Custom("Invalid CoordinatorId".into()));
    }

    let Some(state_account_id) = state.account else {
        return Err(RpcError::Custom("No Account configured".into()));
    };

    if account_id != state_account_id {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    // TODO: Filter assets by those on the same chain as the account
    vault::GetAssets
        .call(transport.clone(), state.vault_id)
        .await
}

async fn propose(
    transport: Arc<JsonRpcTransport>,
    params: (CoordinatorId, AccountId, coordinator::EvmBundle),
) -> Result<(), RpcError> {
    info!("Received proposal: {:#?}", params);

    let state: State = try_get_state(transport.clone()).await?;
    let (coordinator_id, account_id, bundle) = params;

    if Some(coordinator_id.into()) != state.coordinator_id {
        return Err(RpcError::Custom("Invalid CoordinatorId".into()));
    }

    let Some(state_account_id) = state.account.clone() else {
        return Err(RpcError::Custom("No Account configured".into()));
    };

    let Some(state_account_address) = state_account_id.as_evm_address() else {
        return Err(RpcError::Custom("Account is not an EVM account".into()));
    };

    let Some(state_private_key) = state.private_key else {
        return Err(RpcError::Custom("No Private Key configured".into()));
    };

    if account_id != state_account_id {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    let signer = PrivateKeySigner::from_bytes(&state_private_key).map_err(to_rpc_err)?;
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_client(AlloyBridge::new(
            transport.clone(),
            state.provider_id.clone(),
        ));

    let initial_native_balance = provider
        .get_balance(state_account_address)
        .await
        .map_err(to_rpc_err)?;

    verify_vault_balance(&transport, &state, &bundle).await?;

    let return_assets = validate_and_get_return_assets(&transport, &state, &bundle).await?;
    withdraw_assets(transport, state, &state_account_id, &bundle).await?;
    execute_bundle(&provider, bundle).await?;
    return_outstanding_assets(
        &provider,
        state_account_address,
        return_assets,
        initial_native_balance,
    )
    .await?;

    Ok(())
}

async fn verify_vault_balance(
    transport: &Arc<JsonRpcTransport>,
    state: &State,
    bundle: &coordinator::EvmBundle,
) -> Result<(), RpcError> {
    let vault_assets = vault::GetAssets
        .call(transport.clone(), state.vault_id)
        .await?;

    for (asset_id, amount) in &bundle.inputs {
        let vault_amount = vault_assets
            .iter()
            .find_map(|(id, amt)| (id == asset_id).then_some(*amt))
            .unwrap_or(U256::ZERO);

        if &vault_amount < amount {
            return Err(RpcError::Custom(format!(
                "Insufficient asset {asset_id} in vault"
            )));
        }
    }

    Ok(())
}

async fn validate_and_get_return_assets(
    transport: &Arc<JsonRpcTransport>,
    state: &State,
    bundle: &coordinator::EvmBundle,
) -> Result<Vec<ReturnAsset>, RpcError> {
    let mut return_assets: Vec<ReturnAsset> = Vec::new();

    let bundled_assets = bundle
        .inputs
        .iter()
        .map(|f| f.0.clone())
        .chain(bundle.outputs.iter().map(|f| f.clone()));

    for asset_id in bundled_assets {
        if (asset_id.chain_id) != ChainId::new_evm(CHAIN_ID) {
            return Err(RpcError::Custom(format!(
                "Coordinator cannot return asset {} on chain {}",
                asset_id, asset_id.chain_id
            )));
        }

        let asset = match asset_id.asset {
            AssetType::Erc20(address) => EvmAsset::Erc20(address),
            AssetType::Slip44(id) => {
                if id != 60 {
                    return Err(RpcError::Custom(format!(
                        "Coordinator cannot return unsupported slip44 asset {}",
                        asset_id
                    )));
                }
                EvmAsset::Eth
            }
            _ => {
                return Err(RpcError::Custom(format!(
                    "Coordinator cannot return unsupported asset {}",
                    asset_id
                )));
            }
        };

        let deposit_address = vault::GetDepositAddress
            .call(transport.clone(), (state.vault_id, asset_id.clone()))
            .await?;

        let Some(deposit_address) = deposit_address.as_evm_address() else {
            return Err(RpcError::Custom(format!(
                "Coordinator cannot return asset {} to non-EVM address {}",
                asset_id, deposit_address
            )));
        };

        return_assets.push(ReturnAsset {
            asset,
            deposit_address,
        });
    }
    Ok(return_assets)
}

async fn withdraw_assets(
    transport: Arc<JsonRpcTransport>,
    state: State,
    state_account_id: &AccountId,
    bundle: &coordinator::EvmBundle,
) -> Result<(), RpcError> {
    for (asset_id, amount) in &bundle.inputs {
        info!("Transferring asset {} amount {}", asset_id, amount);
        vault::Withdraw
            .call(
                transport.clone(),
                (
                    state.vault_id,
                    state_account_id.clone(),
                    asset_id.clone(),
                    amount.clone(),
                ),
            )
            .await?;
    }

    Ok(())
}

async fn execute_bundle<T: Provider>(
    provider: &T,
    bundle: coordinator::EvmBundle,
) -> Result<(), RpcError> {
    for operation in bundle.operations {
        info!("Submitting operation: {:?}...", operation);
        let tx = TransactionRequest::default()
            .to(operation.to)
            .input(operation.data.into())
            .value(operation.value);
        let tx_hash = provider
            .send_transaction(tx)
            .await
            .map_err(to_rpc_err)?
            .watch()
            .await
            .map_err(to_rpc_err)?;
        info!("Submitted operation with tx_hash {}", tx_hash);
    }

    Ok(())
}

async fn return_outstanding_assets<T: Provider>(
    provider: &T,
    state_account_address: Address,
    return_assets: Vec<ReturnAsset>,
    initial_native_balance: U256,
) -> Result<(), RpcError> {
    for return_asset in return_assets {
        info!("Returning {:?} to vault...", &return_asset.asset);
        match return_asset.asset {
            EvmAsset::Eth => {
                return_eth(
                    provider,
                    state_account_address,
                    return_asset.deposit_address,
                    initial_native_balance,
                )
                .await?;
            }
            EvmAsset::Erc20(address) => {
                return_erc20(
                    provider,
                    state_account_address,
                    return_asset.deposit_address,
                    address,
                )
                .await?;
            }
        }
    }

    Ok(())
}

async fn return_eth<T: Provider>(
    provider: &T,
    state_account_address: Address,
    deposit_address: Address,
    initial_native_balance: U256,
) -> Result<(), RpcError> {
    let balance = provider
        .get_balance(state_account_address)
        .await
        .map_err(to_rpc_err)?;

    //? Return only the excess balance above the initial balance
    //? This means that any ETH remaining will first be used to cover gas costs,
    //? which is generally fine.
    let return_amount = balance.saturating_sub(initial_native_balance);
    if return_amount == U256::ZERO {
        trace!("No balance to return, skipping ETH return");
        return Ok(());
    }

    let tx_hash = provider
        .send_transaction(
            TransactionRequest::default()
                .to(deposit_address)
                .value(return_amount),
        )
        .await
        .map_err(to_rpc_err)?
        .watch()
        .await
        .map_err(to_rpc_err)?;
    info!("Returned ETH to vault with tx_hash {}", tx_hash);
    Ok(())
}

async fn return_erc20<T: Provider>(
    provider: &T,
    state_account_address: Address,
    deposit_address: Address,
    erc20_address: Address,
) -> Result<(), RpcError> {
    let erc20 = ERC20::new(erc20_address, &provider);
    let balance = erc20
        .balanceOf(state_account_address)
        .call()
        .await
        .map_err(to_rpc_err)?;

    if balance == U256::ZERO {
        trace!("No balance for ERC20 {}, skipping return", erc20_address);
        return Ok(());
    }

    let tx_hash = erc20
        .transfer(deposit_address, balance)
        .send()
        .await
        .map_err(to_rpc_err)?
        .watch()
        .await
        .map_err(to_rpc_err)?;
    info!(
        "Returned ERC20 {} to vault with tx_hash {}",
        erc20_address, tx_hash
    );

    Ok(())
}

// ---------- UI Handlers ----------
async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    let component = container(vec![
        heading("EOA Coordinator"),
        text("This is an example dev coordinator plugin."),
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
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    props: (PageId, PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = props;

    let private_key_hex = match event {
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "private_key_form" => {
            let Some(pk) = form_data.get("dev_private_key") else {
                return Err(RpcError::Custom("Private key not found in form".into()));
            };
            pk.clone()
        }
        page::PageEvent::ButtonClicked(button_id) if button_id == "generate_dev_key" => {
            let signer = PrivateKeySigner::random();
            let private_key = signer.to_bytes();
            hex::encode(private_key)
        }
        _ => {
            return Ok(());
        }
    };

    let signer: PrivateKeySigner = private_key_hex
        .parse()
        .map_err(|_| RpcError::Custom("Invalid private key".into()))?;
    let address = signer.address();
    let account_id = AccountId::new_evm(CHAIN_ID, address);

    let coordinator_id = host::RegisterEntity
        .call(transport.clone(), Domain::Coordinator)
        .await?;

    let mut state: State = try_get_state(transport.clone()).await?;
    state.coordinator_id = Some(coordinator_id);
    state.private_key = Some(signer.to_bytes());
    state.account = Some(account_id.clone());
    set_state(transport.clone(), &state).await?;

    let component = container(vec![
        heading("Coordinator"),
        text(&format!("Address: {}", address)),
        text(&format!("Private Key: {}", private_key_hex)),
    ]);
    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting plugin...");

    PluginServer::new_with_transport()
        .with_method(global::Ping, ping)
        .with_method(plugin::Init, init)
        .with_method(coordinator::GetSession, get_session)
        .with_method(coordinator::GetAssets, get_assets)
        .with_method(coordinator::Propose, propose)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .run();
}
