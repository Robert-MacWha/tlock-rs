use std::{io::stderr, sync::Arc, task::Poll};

use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, TxHash, U256},
    providers::{Provider, ProviderBuilder},
    rpc::{
        client::RpcClient,
        types::{
            Block, BlockOverrides, BlockTransactionsKind, Filter, Log, Transaction,
            TransactionReceipt, TransactionRequest, state::StateOverride,
        },
    },
    transports::{TransportError, TransportErrorKind, TransportFut},
};
use alloy_json_rpc::{RequestPacket, ResponsePacket};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    futures::executor::block_on,
    server::ServerBuilder,
    state::{get_state, set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        component::{container, text},
        domains::Domain,
        entities::{EthProviderId, PageId},
        eth, global, host, page, plugin,
    },
    wasmi_pdk::{
        rpc_message::RpcError,
        tracing::{error, info},
        tracing_subscriber::fmt,
        transport::JsonRpcTransport,
    },
};
use tower_service::Service;

#[derive(Serialize, Deserialize, Default)]
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
        rpc_url: "https://eth.llamarpc.com".to_string(),
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

    host::SetInterface
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
        Option<BlockOverrides>,
        Option<StateOverride>,
    ),
) -> Result<Bytes, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;

    let (_provider_id, tx, block_overrides, state_overrides) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let resp = provider
        .call(tx)
        .with_block_overrides_opt(block_overrides)
        .overrides_opt(state_overrides)
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
) -> Result<U256, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let gas_price = provider.get_gas_price().await.map_err(|e| {
        error!("Error fetching gas price: {:?}", e);
        RpcError::Custom(format!("Failed to fetch gas price: {:?}", e))
    })?;
    let gas_price = U256::from(gas_price);

    Ok(gas_price)
}

async fn get_balance(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let state: ProviderState = try_get_state(transport.clone()).await?;
    let (_provider_id, address, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let balance = provider
        .get_balance(address)
        .block_id(block_id)
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

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let transport = Arc::new(transport);

    let plugin = ServerBuilder::new(transport.clone())
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
        .with_method(eth::SendRawTransaction, send_raw_transaction)
        .finish();
    let plugin = Arc::new(plugin);

    block_on(async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    });
}

//? Helpers to create an alloy provider using the host transport and `Request`
// instead of a  standard HTTP transport.
pub fn create_alloy_provider(
    transport: Arc<JsonRpcTransport>,
    url: String,
) -> impl alloy::providers::Provider {
    let host_transport = HostTransportService::new(transport, url);
    let client = RpcClient::new(host_transport, false);

    ProviderBuilder::new().connect_client(client)
}

#[derive(Clone)]
pub struct HostTransportService {
    transport: Arc<JsonRpcTransport>,
    rpc_url: String,
}

impl HostTransportService {
    pub fn new(transport: Arc<JsonRpcTransport>, rpc_url: String) -> Self {
        Self { transport, rpc_url }
    }
}

impl Service<RequestPacket> for HostTransportService {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        let transport = self.transport.clone();
        let rpc_url = self.rpc_url.clone();
        Box::pin(async move {
            let mut params = host::Request {
                url: rpc_url,
                method: "POST".to_string(),
                headers: req
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.as_bytes().into()))
                    .collect(),
                body: Some(serde_json::to_vec(&req).map_err(TransportErrorKind::custom)?),
            };
            params.headers.push((
                "Content-Type".to_string(),
                "application/json".as_bytes().into(),
            ));

            let resp = host::Fetch
                .call(transport, params)
                .await
                .map_err(TransportErrorKind::custom)?;

            let Ok(body) = resp else {
                return Err(TransportErrorKind::custom_str(&resp.err().unwrap()));
            };

            serde_json::from_slice(&body)
                .map_err(|err| TransportError::deser_err(err, String::from_utf8_lossy(&body)))
        })
    }

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
