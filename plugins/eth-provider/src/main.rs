use crate::alloy_provider::create_alloy_provider;
use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, TxHash, U256},
    providers::Provider,
    rpc::types::{
        Block, BlockOverrides, BlockTransactionsKind, Filter, Log, Transaction, TransactionReceipt,
        TransactionRequest, state::StateOverride,
    },
};
use serde::{Deserialize, Serialize};
use std::{io::stderr, sync::Arc};
use tlock_pdk::{
    server::PluginServer,
    state::{get_state, set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        component::{container, text},
        domains::Domain,
        entities::{EthProviderId, PageId},
        eth, global, host, page, plugin,
    },
    wasmi_plugin_pdk::{
        rpc_message::RpcError,
        tracing::{error, info},
        tracing_subscriber::fmt,
        transport::JsonRpcTransport,
    },
};

mod alloy_provider;

#[derive(Serialize, Deserialize, Default, Debug)]
struct ProviderState {
    rpc_url: String,
}

async fn ping(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<String, RpcError> {
    global::Ping.call(transport.clone(), ()).await?;
    Ok("pong".to_string())
}

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    info!("Initializing Ethereum Provider Plugin...");

    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    info!("Registering Ethereum Provider...");

    host::RegisterEntity
        .call(transport.clone(), Domain::EthProvider)
        .await?;

    let state = ProviderState {
        rpc_url: "https://1rpc.io/sepolia".to_string(),
    };
    set_state(transport.clone(), &state).await?;

    Ok(())
}

async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    let state: ProviderState = get_state(transport.clone()).await;

    let component = container(vec![
        text("This is the Ethereum Provider Plugin"),
        text(format!("RPC URL: {}", state.rpc_url)),
    ]);

    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn chain_id(
    transport: Arc<JsonRpcTransport>,
    _params: EthProviderId,
) -> Result<U256, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let chain_id = provider.get_chain_id().await.map_err(|e| {
        error!("Error fetching chain ID: {:?}", e);
        RpcError::InternalError
    })?;
    let chain_id = U256::from(chain_id);

    Ok(chain_id)
}

async fn block_number(
    transport: Arc<JsonRpcTransport>,
    _params: EthProviderId,
) -> Result<u64, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let block_number = provider.get_block_number().await.map_err(|e| {
        error!("Error fetching block number: {:?}", e);
        RpcError::InternalError
    })?;
    Ok(block_number)
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
    let state: ProviderState = try_get_state(transport.clone()).await?;

    let (_provider_id, tx, block, state_overrides, block_overrides) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let resp = provider
        .call(tx)
        .block(block)
        .overrides_opt(state_overrides)
        .with_block_overrides_opt(block_overrides)
        .await
        .map_err(|e| {
            error!("Error processing call: {:?}", e);
            RpcError::Custom(format!("Call failed: {:?}", e))
        })?;

    Ok(resp)
}

async fn gas_price(
    transport: Arc<JsonRpcTransport>,
    _provider_id: EthProviderId,
) -> Result<u128, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let gas_price = provider.get_gas_price().await.map_err(|e| {
        error!("Error fetching gas price: {:?}", e);
        RpcError::Custom(format!("Failed to fetch gas price: {:?}", e))
    })?;

    Ok(gas_price)
}

async fn get_balance(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, address, block) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let balance = provider
        .get_balance(address)
        .block_id(block)
        .await
        .map_err(|e| {
            error!("Error fetching balance: {:?}", e);
            RpcError::Custom(format!("Failed to fetch balance: {:?}", e))
        })?;
    Ok(balance)
}

async fn get_block(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, BlockId, BlockTransactionsKind),
) -> Result<Block, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, block_id, include_transactions) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let block = provider
        .get_block(block_id)
        .kind(include_transactions)
        .await
        .map_err(|e| {
            error!("Error fetching block: {:?}", e);
            RpcError::Custom(format!("Failed to fetch block: {:?}", e))
        })?;

    match block {
        Some(b) => Ok(b),
        None => Err(RpcError::InternalError),
    }
}

async fn get_block_receipts(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, BlockId),
) -> Result<Vec<TransactionReceipt>, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let receipts = provider.get_block_receipts(block_id).await.map_err(|e| {
        error!("Error fetching block receipts: {:?}", e);
        RpcError::Custom(format!("Failed to fetch block receipts: {:?}", e))
    })?;

    match receipts {
        Some(r) => Ok(r),
        None => Ok(vec![]),
    }
}

async fn get_code(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<Bytes, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, address, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let code = provider
        .get_code_at(address)
        .block_id(block_id)
        .await
        .map_err(|e| {
            error!("Error fetching code: {:?}", e);
            RpcError::Custom(format!("Failed to fetch code: {:?}", e))
        })?;

    Ok(code)
}

async fn get_logs(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Filter),
) -> Result<Vec<Log>, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, filter) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let logs = provider.get_logs(&filter).await.map_err(|e| {
        error!("Error fetching logs: {:?}", e);
        RpcError::Custom(format!("Failed to fetch logs: {:?}", e))
    })?;

    Ok(logs)
}

async fn get_transaction_by_hash(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, TxHash),
) -> Result<Transaction, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, tx_hash) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let tx = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .map_err(|e| {
            error!("Error fetching transaction: {:?}", e);
            RpcError::Custom(format!("Failed to fetch transaction: {:?}", e))
        })?;

    match tx {
        Some(t) => Ok(t),
        None => Err(RpcError::InternalError),
    }
}

async fn get_transaction_receipt(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, TxHash),
) -> Result<TransactionReceipt, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, tx_hash) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(|e| {
            error!("Error fetching transaction receipt: {:?}", e);
            RpcError::Custom(format!("Failed to fetch transaction receipt: {:?}", e))
        })?;

    match receipt {
        Some(r) => Ok(r),
        None => Err(RpcError::InternalError),
    }
}

async fn get_transaction_count(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<u64, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, address, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let tx_count = provider
        .get_transaction_count(address)
        .block_id(block_id)
        .await
        .map_err(|e| {
            error!("Error fetching transaction count: {:?}", e);
            RpcError::Custom(format!("Failed to fetch transaction count: {:?}", e))
        })?;

    Ok(tx_count)
}

async fn send_raw_transaction(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Bytes),
) -> Result<TxHash, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, raw_tx) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let tx = provider.send_raw_transaction(&raw_tx).await.map_err(|e| {
        error!("Error sending raw transaction: {:?}", e);
        RpcError::Custom(format!("Failed to send raw transaction: {:?}", e))
    })?;
    let tx_hash = tx.tx_hash();

    Ok(*tx_hash)
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
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, transaction_request, block_id, state_override, block_override) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let gas_estimate = provider
        .estimate_gas(transaction_request)
        .block(block_id)
        .overrides_opt(state_override)
        .with_block_overrides_opt(block_override)
        .await
        .map_err(|e| {
            error!("Error estimating gas: {:?}", e);
            RpcError::Custom(format!("Failed to estimate gas: {:?}", e))
        })?;

    Ok(gas_estimate)
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
        .with_method(page::OnLoad, on_load)
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
        .run();
}
