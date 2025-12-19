use std::{io::stderr, sync::Arc};

use revm::{
    DatabaseRef,
    primitives::{Address, Bytes, alloy_primitives::TxHash},
};
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    server::PluginServer,
    state::{set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        alloy::{
            eips::BlockId,
            primitives::U256,
            providers::{Provider as AlloyProvider, ProviderBuilder},
            rpc::types::{
                Block, BlockOverrides, BlockTransactionsKind, Transaction, TransactionReceipt,
                TransactionRequest, state::StateOverride,
            },
        },
        caip::ChainId,
        domains::Domain,
        entities::EthProviderId,
        eth::{self},
        host, plugin,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, to_rpc_err},
        transport::JsonRpcTransport,
    },
};
use tracing::info;
use tracing_subscriber::fmt;

use crate::{
    alloydb::AlloyDb, provider::Provider, provider_snapshot::ProviderSnapshot,
    rpc::header_to_block_env,
};

mod alloydb;
mod chain;
mod provider;
mod provider_snapshot;
mod rpc;

#[derive(Debug, Serialize, Deserialize)]
struct State {
    alloy_provider_id: EthProviderId,
    fork_block: BlockId,

    fork_snapshot: ProviderSnapshot,
}

const CHAIN_ID: u64 = 11155111u64;

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    // TODO: Consider embedding the alloy instance in this plugin rather than using
    // an eth provider. Saves the cost of inter-plugin calls.
    let provider_id = host::RequestEthProvider
        .call(transport.clone(), ChainId::new_evm(CHAIN_ID))
        .await?;

    //? Setup the forked provider
    let alloy =
        ProviderBuilder::new().connect_client(AlloyBridge::new(transport.clone(), provider_id));

    let block = alloy
        .get_block(BlockId::latest())
        .await
        .map_err(to_rpc_err)?
        .ok_or(RpcError::Custom("Failed to get latest block".into()))?;
    let fork_block_id = BlockId::number(block.number());

    let db = AlloyDb::new(alloy, fork_block_id)
        .ok_or(RpcError::Custom("No tokio runtime available".into()))?;
    let parent_hash = block.header.parent_hash;
    let block_env = header_to_block_env(block.header);
    let fork = Provider::new(db, block_env, parent_hash);
    let fork_snapshot = fork.snapshot();

    //? Save setup snapshot
    let state = State {
        alloy_provider_id: provider_id,
        fork_block: fork_block_id,
        fork_snapshot,
    };
    set_state(transport.clone(), &state).await?;

    //? Register the revm eth provider
    host::RegisterEntity
        .call(transport.clone(), Domain::EthProvider)
        .await?;

    Ok(())
}

async fn chain_id(_: Arc<JsonRpcTransport>, _: EthProviderId) -> Result<U256, RpcError> {
    Ok(U256::from(CHAIN_ID))
}

async fn block_number(transport: Arc<JsonRpcTransport>, _: EthProviderId) -> Result<u64, RpcError> {
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.block_number())
}

async fn gas_price(transport: Arc<JsonRpcTransport>, _: EthProviderId) -> Result<u128, RpcError> {
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.gas_price())
}

async fn get_balance(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let (_, address, block_id) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_balance(address, block_id)?)
}

async fn get_block(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, BlockId, BlockTransactionsKind),
) -> Result<Block, RpcError> {
    let (_, block_id, tx_kind) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_block(block_id, tx_kind)?)
}

async fn get_code(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<Bytes, RpcError> {
    let (_, address, block_id) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    let code = fork
        .get_code(address, block_id)?
        .ok_or(RpcError::Custom("Account has no code".into()))?;
    Ok(code)
}

async fn get_transaction_count(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<u64, RpcError> {
    let (_, address, block_id) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_transaction_count(address, block_id)?)
}

async fn get_transaction_by_hash(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, TxHash),
) -> Result<Transaction, RpcError> {
    let (_, tx_hash) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_transaction_by_hash(tx_hash)?)
}

async fn get_transaction_receipt(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, TxHash),
) -> Result<TransactionReceipt, RpcError> {
    let (_, tx_hash) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    let transaction_receipt = fork
        .get_transaction_receipt(tx_hash)
        .ok_or(RpcError::Custom("Transaction receipt not found".into()))?;
    Ok(transaction_receipt)
}

async fn get_block_receipts(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, BlockId),
) -> Result<Vec<TransactionReceipt>, RpcError> {
    let (_, block_id) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_block_receipts(block_id)?)
}

async fn call(
    transport: Arc<JsonRpcTransport>,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<Bytes, RpcError> {
    let (_, tx_request, block_id, state_override, block_override) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.call(tx_request, block_id, state_override, block_override)?)
}

async fn estimate_gas(
    transport: Arc<JsonRpcTransport>,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<u64, RpcError> {
    let (_, tx_request, block_id, state_override, block_override) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.estimate_gas(tx_request, block_id, state_override, block_override)?)
}

async fn send_raw_transaction(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Bytes),
) -> Result<TxHash, RpcError> {
    let (_, raw_tx) = params;
    let mut fork = get_fork_provider(transport.clone()).await?;
    let tx = fork.send_raw_transaction(raw_tx)?;

    // TODO: Save the fork database state

    Ok(tx)
}

/// Returns a fork provider based on the saved state.
///
/// Errors if no tokio runtime is available.
async fn get_fork_provider(
    transport: Arc<JsonRpcTransport>,
) -> Result<Provider<impl DatabaseRef>, RpcError> {
    let state: State = try_get_state(transport.clone()).await.unwrap();
    let alloy_provider_id = state.alloy_provider_id;
    let fork_block = state.fork_block;
    let fork_snapshot = state.fork_snapshot;

    let alloy = ProviderBuilder::new()
        .connect_client(AlloyBridge::new(transport.clone(), alloy_provider_id));

    let db = AlloyDb::new(alloy, fork_block)
        .ok_or(RpcError::Custom("No tokio runtime available".into()))?;
    let fork_provider = Provider::from_snapshot(db, fork_snapshot);
    Ok(fork_provider)
}

// TODO: See if there's a way to use anvil instead of revm directly + my own
// semi-hacky impl.
fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();

    info!("Starting plugin...");
    PluginServer::new_with_transport()
        .with_method(plugin::Init, init)
        // .with_method(page::OnLoad, on_load)
        .with_method(eth::ChainId, chain_id)
        .with_method(eth::BlockNumber, block_number)
        .with_method(eth::GasPrice, gas_price)
        .with_method(eth::GetBalance, get_balance)
        .with_method(eth::GetBlock, get_block)
        .with_method(eth::GetCode, get_code)
        .with_method(eth::GetTransactionCount, get_transaction_count)
        .with_method(eth::GetTransactionByHash, get_transaction_by_hash)
        .with_method(eth::GetTransactionReceipt, get_transaction_receipt)
        .with_method(eth::GetBlockReceipts, get_block_receipts)
        .with_method(eth::Call, call)
        .with_method(eth::EstimateGas, estimate_gas)
        .with_method(eth::SendRawTransaction, send_raw_transaction)
        // .with_method(eth::GetLogs, get_logs)
        .run();
}
