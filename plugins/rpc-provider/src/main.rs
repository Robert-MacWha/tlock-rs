use std::io::stderr;

use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    primitives::{Address, Bytes, TxHash, U256},
    providers::Provider,
    rpc::types::{
        Block, BlockOverrides, BlockTransactionsKind, Filter, Log, Transaction, TransactionReceipt,
        TransactionRequest, state::StateOverride,
    },
};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    runner::PluginRunner,
    state::{set_state, try_get_state},
    tlock_api::{RpcMethod, domains::Domain, entities::EthProviderId, eth, global, host, plugin},
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, ToRpcResult},
        transport::Transport,
    },
};
use tracing::info;
use tracing_subscriber::fmt;

use crate::alloy_provider::create_alloy_provider;

mod alloy_provider;

#[derive(Serialize, Deserialize, Default, Debug)]
struct ProviderState {
    rpc_url: String,
}

async fn ping(transport: Transport, _params: ()) -> Result<String, RpcError> {
    global::Ping.call_async(transport.clone(), ()).await?;
    Ok("pong".to_string())
}

async fn init(transport: Transport, _params: ()) -> Result<(), RpcError> {
    info!("Initializing Ethereum Provider Plugin...");

    let state = ProviderState {
        rpc_url: "https://1rpc.io/eth".to_string(),
    };
    set_state(transport.clone(), &state)?;

    host::RegisterEntity
        .call_async(transport.clone(), Domain::EthProvider)
        .await?;

    Ok(())
}

async fn chain_id(transport: Transport, _params: EthProviderId) -> Result<U256, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let chain_id = provider.get_chain_id().await.rpc_err()?;
    let chain_id = U256::from(chain_id);

    Ok(chain_id)
}

async fn block_number(transport: Transport, _params: EthProviderId) -> Result<u64, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let block_number = provider.get_block_number().await.rpc_err()?;
    Ok(block_number)
}

async fn call(
    transport: Transport,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<Bytes, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;

    let (_provider_id, tx, block, state_overrides, block_overrides) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let resp = provider
        .call(tx)
        .block(block)
        .overrides_opt(state_overrides)
        .with_block_overrides_opt(block_overrides)
        .await
        .rpc_err()?;

    Ok(resp)
}

async fn gas_price(transport: Transport, _provider_id: EthProviderId) -> Result<u128, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let gas_price = provider.get_gas_price().await.rpc_err()?;

    Ok(gas_price)
}

async fn get_balance(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, address, block) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let balance = provider
        .get_balance(address)
        .block_id(block)
        .await
        .rpc_err()?;
    Ok(balance)
}

async fn get_block(
    transport: Transport,
    params: (EthProviderId, BlockId, BlockTransactionsKind),
) -> Result<Block, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, block_id, include_transactions) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let block = provider
        .get_block(block_id)
        .kind(include_transactions)
        .await
        .rpc_err()?;

    match block {
        Some(b) => Ok(b),
        None => Err(RpcError::Custom("Block not found".into())),
    }
}

async fn get_block_receipts(
    transport: Transport,
    params: (EthProviderId, BlockId),
) -> Result<Vec<TransactionReceipt>, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let receipts = provider.get_block_receipts(block_id).await.rpc_err()?;

    match receipts {
        Some(r) => Ok(r),
        None => Ok(vec![]),
    }
}

async fn get_code(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<Bytes, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, address, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let code = provider
        .get_code_at(address)
        .block_id(block_id)
        .await
        .rpc_err()?;

    Ok(code)
}

async fn get_logs(
    transport: Transport,
    params: (EthProviderId, Filter),
) -> Result<Vec<Log>, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, filter) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let logs = provider.get_logs(&filter).await.rpc_err()?;

    Ok(logs)
}

async fn get_transaction_by_hash(
    transport: Transport,
    params: (EthProviderId, TxHash),
) -> Result<Transaction, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, tx_hash) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let tx = provider.get_transaction_by_hash(tx_hash).await.rpc_err()?;

    match tx {
        Some(t) => Ok(t),
        None => Err(RpcError::Custom("Transaction not found".into())),
    }
}

async fn get_transaction_receipt(
    transport: Transport,
    params: (EthProviderId, TxHash),
) -> Result<TransactionReceipt, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, tx_hash) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let receipt = provider.get_transaction_receipt(tx_hash).await.rpc_err()?;

    match receipt {
        Some(r) => Ok(r),
        None => Err(RpcError::Custom("Transaction Receipt not Found".into())),
    }
}

async fn get_transaction_count(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<u64, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, address, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let tx_count = provider
        .get_transaction_count(address)
        .block_id(block_id)
        .await
        .rpc_err()?;

    Ok(tx_count)
}

async fn send_raw_transaction(
    transport: Transport,
    params: (EthProviderId, Bytes),
) -> Result<TxHash, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, raw_tx) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let tx = provider.send_raw_transaction(&raw_tx).await.rpc_err()?;
    let tx_hash = tx.tx_hash();

    Ok(*tx_hash)
}

async fn estimate_gas(
    transport: Transport,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<u64, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, transaction_request, block_id, state_override, block_override) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let gas_estimate = provider
        .estimate_gas(transaction_request)
        .block(block_id)
        .overrides_opt(state_override)
        .with_block_overrides_opt(block_override)
        .await
        .rpc_err()?;

    Ok(gas_estimate)
}

async fn get_storage_at(
    transport: Transport,
    params: (EthProviderId, Address, U256, BlockId),
) -> Result<U256, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, address, slot, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let storage_value = provider
        .get_storage_at(address, slot)
        .block_id(block_id)
        .await
        .rpc_err()?;

    Ok(storage_value)
}

async fn fee_history(
    transport: Transport,
    params: (EthProviderId, u64, BlockNumberOrTag, Vec<f64>),
) -> Result<alloy::rpc::types::FeeHistory, RpcError> {
    let state: ProviderState = try_get_state(transport.clone())?;
    let (_provider_id, block_count, newest_block, reward_percentiles) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let fee_history = provider
        .get_fee_history(block_count, newest_block, &reward_percentiles)
        .await
        .rpc_err()?;

    Ok(fee_history)
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
        .with_method(eth::ChainId, chain_id)
        .with_method(eth::BlockNumber, block_number)
        .with_method(eth::Call, call)
        .with_method(eth::GasPrice, gas_price)
        .with_method(eth::GetBalance, get_balance)
        .with_method(eth::GetBlock, get_block)
        .with_method(eth::GetBlockReceipts, get_block_receipts)
        .with_method(eth::GetCode, get_code)
        .with_method(eth::GetLogs, get_logs)
        .with_method(eth::GetTransactionByHash, get_transaction_by_hash)
        .with_method(eth::GetTransactionReceipt, get_transaction_receipt)
        .with_method(eth::GetTransactionCount, get_transaction_count)
        .with_method(eth::SendRawTransaction, send_raw_transaction)
        .with_method(eth::EstimateGas, estimate_gas)
        .with_method(eth::GetStorageAt, get_storage_at)
        .with_method(eth::FeeHistory, fee_history)
        .run();
}
