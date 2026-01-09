//! Basic EOA Coordinator Plugin implementation.
//!
//! NOT DESIGNED FOR PRODUCTION USE.
//!
//! This is a minimal, very insecure implementation of a Coordinator Plugin. It
//! stores its private key in memory and in plaintext host storage, does not
//! authenticate requests, and does not do any validation of incoming data. It
//! is intended purely for demonstration and testing.
use std::io::stderr;

use alloy::{
    primitives::{Address, FixedBytes},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
};
use erc20s::CHAIN_ID;
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    runner::PluginRunner,
    state::{set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        alloy::primitives::U256,
        caip::{AccountId, AssetId, AssetType, ChainId},
        coordinator,
        domains::Domain,
        entities::{CoordinatorId, EntityId, EthProviderId, VaultId},
        global, host, plugin, vault,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, RpcErrorContext, ToRpcResult},
        transport::Transport,
    },
};
use tracing::{error, info};
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct State {
    /// Vault managed by this coordinator
    vault_id: VaultId,
    provider_id: EthProviderId,
    coordinator: Coordinator,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Coordinator {
    entity_id: EntityId,
    private_key: FixedBytes<32>,
    account: AccountId,
}

sol! {
    #[sol(rpc)]
    contract ERC20 {
        function balanceOf(address owner) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

/// Minimum gas required for executing a bundle
/// TODO: Dynamically calculate based on bundle complexity
const REQUIRED_GAS: u128 = 10000000000000000; // 0.01 ETH

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

async fn ping(transport: Transport, _: ()) -> Result<String, RpcError> {
    global::Ping.call_async(transport, ()).await?;
    Ok("pong".to_string())
}

async fn init(transport: Transport, _: ()) -> Result<(), RpcError> {
    let provider_id = host::RequestEthProvider
        .call_async(transport.clone(), ChainId::new_evm(CHAIN_ID))
        .await?;
    let vault_id = host::RequestVault.call_async(transport.clone(), ()).await?;

    let coordinator_id = host::RegisterEntity
        .call_async(transport.clone(), Domain::Coordinator)
        .await?;

    let signer = PrivateKeySigner::random();
    let address = signer.address();
    let account_id = AccountId::new_evm(CHAIN_ID, address);

    let state = State {
        vault_id,
        provider_id,
        coordinator: Coordinator {
            entity_id: coordinator_id,
            private_key: signer.to_bytes(),
            account: account_id,
        },
    };

    set_state(transport.clone(), &state)?;
    Ok(())
}

async fn get_session(
    transport: Transport,
    params: (CoordinatorId, ChainId, Option<AccountId>),
) -> Result<AccountId, RpcError> {
    let state: State = try_get_state(transport.clone())?;
    let (coordinator_id, chain_id, maybe_account_id) = params;

    let coordinator_id: EntityId = coordinator_id.into();
    if coordinator_id != state.coordinator.entity_id {
        return Err(RpcError::custom("Invalid CoordinatorId"));
    }

    // TODO: Support arbitrary evm chain IDs
    if chain_id != ChainId::new_evm(CHAIN_ID) {
        return Err(RpcError::Custom("Invalid ChainId".into()));
    }

    if let Some(account_id) = maybe_account_id
        && account_id != state.coordinator.account
    {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    Ok(state.coordinator.account.clone())
}

async fn get_assets(
    transport: Transport,
    params: (CoordinatorId, AccountId),
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let state: State = try_get_state(transport.clone())?;
    let (coordinator_id, account_id) = params;

    let coordinator_id: EntityId = coordinator_id.into();
    if coordinator_id != state.coordinator.entity_id {
        return Err(RpcError::Custom("Invalid CoordinatorId".into()));
    }

    if account_id != state.coordinator.account {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    // TODO: Filter assets by those on the same chain as the account
    Ok(vault::GetAssets
        .call_async(transport.clone(), state.vault_id)
        .await?)
}

async fn propose(
    transport: Transport,
    params: (CoordinatorId, AccountId, coordinator::EvmBundle),
) -> Result<(), RpcError> {
    info!("Received proposal: {:?}", params);

    let state: State = try_get_state(transport.clone())?;
    let (coordinator_id, account_id, bundle) = params;
    let coordinator = state.coordinator.clone();

    let coordinator_id: EntityId = coordinator_id.into();
    if coordinator_id != coordinator.entity_id {
        return Err(RpcError::custom("Invalid CoordinatorId"));
    }

    if account_id != coordinator.account {
        return Err(RpcError::custom("Invalid AccountId"));
    }

    let signer =
        PrivateKeySigner::from_bytes(&coordinator.private_key).context("Invalid private key")?;
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_client(AlloyBridge::new(transport.clone(), state.provider_id));

    let evm_address = match coordinator.account.as_evm_address() {
        Some(addr) => addr,
        None => {
            return Err(RpcError::Custom(
                "Coordinator account is not an EVM address".into(),
            ));
        }
    };

    let initial_native_balance = provider.get_balance(evm_address).await.rpc_err()?;
    verify_vault_balance(&transport, &state, &bundle).await?;

    let return_assets = validate_and_get_return_assets(transport.clone(), &state, &bundle).await?;
    withdraw_gas(
        &provider,
        transport.clone(),
        &state,
        &coordinator.account,
        U256::from(REQUIRED_GAS),
    )
    .await?;
    withdraw_assets(transport.clone(), &state, &coordinator.account, &bundle).await?;
    //? Log the error, but continue to the return step regardless
    let _ = execute_bundle(&provider, bundle).await.map_err(|e| {
        // TODO: Notify host on error
        error!("Error executing bundle: {:?}", e);
    });
    return_outstanding_assets(
        &provider,
        evm_address,
        return_assets,
        initial_native_balance,
    )
    .await?;

    Ok(())
}

async fn verify_vault_balance(
    transport: &Transport,
    state: &State,
    bundle: &coordinator::EvmBundle,
) -> Result<(), RpcError> {
    let vault_assets = vault::GetAssets
        .call_async(transport.clone(), state.vault_id)
        .await?;

    for (asset_id, amount) in &bundle.inputs {
        let vault_amount = vault_assets
            .iter()
            .find_map(|(id, amt)| (id == asset_id).then_some(*amt))
            .unwrap_or(U256::ZERO);

        if &vault_amount < amount {
            return Err(RpcError::Custom(format!(
                "Insufficient asset {asset_id} in vault {} ({} < {})",
                state.vault_id, vault_amount, amount
            )));
        }
    }

    Ok(())
}

async fn validate_and_get_return_assets(
    transport: Transport,
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
            .call_async(transport.clone(), (state.vault_id, asset_id.clone()))
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

async fn withdraw_gas<T: Provider>(
    provider: &T,
    transport: Transport,
    state: &State,
    state_account_id: &AccountId,
    required_gas: U256,
) -> Result<(), RpcError> {
    let balance = provider
        .get_balance(state_account_id.as_evm_address().unwrap())
        .await
        .rpc_err()?;

    let required_gas = required_gas.saturating_sub(balance);
    if required_gas == U256::ZERO {
        info!("Sufficient gas balance available, no withdrawal needed");
        return Ok(());
    }

    info!("Withdrawing gas from vault: {}...", required_gas);
    let eth_asset_id = AssetId::eth(CHAIN_ID);
    vault::Withdraw
        .call_async(
            transport.clone(),
            (
                state.vault_id,
                state_account_id.clone(),
                eth_asset_id,
                required_gas,
            ),
        )
        .await?;

    Ok(())
}

async fn withdraw_assets(
    transport: Transport,
    state: &State,
    state_account_id: &AccountId,
    bundle: &coordinator::EvmBundle,
) -> Result<(), RpcError> {
    for (asset_id, amount) in &bundle.inputs {
        info!("Withdrawing from vault: {}:{}...", asset_id, amount);
        vault::Withdraw
            .call_async(
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
            .rpc_err()?
            .watch()
            .await
            .rpc_err()?;
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
        info!("Returning to vault: {:?}...", &return_asset.asset);
        match return_asset.asset {
            EvmAsset::Eth => {
                let _ = return_eth(
                    provider,
                    state_account_address,
                    return_asset.deposit_address,
                    initial_native_balance,
                )
                .await
                .map_err(|e| error!("Error returning {:?}: {}", &return_asset.asset, e));
            }
            EvmAsset::Erc20(address) => {
                let _ = return_erc20(
                    provider,
                    state_account_address,
                    return_asset.deposit_address,
                    address,
                )
                .await
                .map_err(|e| error!("Error returning {:?}: {}", &return_asset.asset, e));
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
        .rpc_err()?;

    //? Return only the excess balance above the initial balance
    //? This means that any ETH remaining will first be used to cover gas costs,
    //? which is generally fine.
    let return_amount = balance.saturating_sub(initial_native_balance);
    if return_amount == U256::ZERO {
        info!("No balance to return, skipping ETH return");
        return Ok(());
    }

    let tx_hash = provider
        .send_transaction(
            TransactionRequest::default()
                .to(deposit_address)
                .value(return_amount),
        )
        .await
        .rpc_err()?
        .watch()
        .await
        .rpc_err()?;
    info!(
        "Returned {} ETH to vault with tx_hash {}",
        return_amount, tx_hash
    );
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
        .rpc_err()?;

    if balance == U256::ZERO {
        info!("No balance for ERC20 {}, skipping return", erc20_address);
        return Ok(());
    }

    let tx_hash = erc20
        .transfer(deposit_address, balance)
        .send()
        .await
        .rpc_err()?
        .watch()
        .await
        .rpc_err()?;
    info!(
        "Returned {} ERC20 {} to vault with tx_hash {}",
        balance, erc20_address, tx_hash
    );

    Ok(())
}

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();

    PluginRunner::new()
        .with_method(global::Ping, ping)
        .with_method(plugin::Init, init)
        .with_method(coordinator::GetSession, get_session)
        .with_method(coordinator::GetAssets, get_assets)
        .with_method(coordinator::Propose, propose)
        .run();
}
