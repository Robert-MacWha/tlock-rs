use std::{io::stderr, sync::Arc, task::Poll};

use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::{
        client::RpcClient,
        types::{BlockOverrides, TransactionRequest, state::StateOverride},
    },
    transports::{TransportError, TransportErrorKind, TransportFut},
};
use alloy_json_rpc::{RequestPacket, ResponsePacket};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    futures::executor::block_on,
    server::ServerBuilder,
    state::{get_state, set_state},
    tlock_api::{
        RpcMethod,
        component::{container, text},
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
    let page_id = PageId::new("eth_provider_page".to_string());
    host::RegisterEntity
        .call(transport.clone(), page_id.into())
        .await?;

    info!("Registering Ethereum Provider...");

    let provider_id = EthProviderId::new("eth_provider".to_string());
    host::RegisterEntity
        .call(transport.clone(), provider_id.into())
        .await?;

    let state = ProviderState {
        rpc_url: "https://eth.llamarpc.com".to_string(),
    };
    set_state(transport.clone(), &state).await?;

    Ok(())
}

async fn on_load(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, u32),
) -> Result<(), RpcError> {
    let (_page_id, interface_id) = params;
    let state: ProviderState = get_state(transport.clone()).await.unwrap_or_default();

    let component = container(vec![
        text("This is the Ethereum Provider Plugin").into(),
        text(format!("RPC URL: {}", state.rpc_url)).into(),
    ]);

    host::SetInterface
        .call(transport.clone(), (interface_id, component))
        .await?;

    return Ok(());
}

async fn block_number(
    transport: Arc<JsonRpcTransport>,
    _params: EthProviderId,
) -> Result<u64, RpcError> {
    let state: ProviderState = get_state(transport.clone()).await?;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let block_number = provider.get_block_number().await.map_err(|e| {
        error!("Error fetching block number: {:?}", e);
        RpcError::InternalError
    })?;
    return Ok(block_number);
}

async fn get_balance(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let state: ProviderState = get_state(transport.clone()).await?;
    let (_provider_id, address, block_id) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let balance = provider
        .get_balance(address)
        .block_id(block_id)
        .await
        .map_err(|e| {
            error!("Error fetching balance: {:?}", e);
            RpcError::InternalError
        })?;
    return Ok(balance);
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
    let state: ProviderState = get_state(transport.clone()).await?;

    let (_provider_id, tx, block_overrides, state_overrides) = params;

    let provider = create_alloy_provider(transport.clone(), state.rpc_url);
    let resp = provider
        .call(tx)
        .with_block_overrides_opt(block_overrides)
        .overrides_opt(state_overrides)
        .await
        .map_err(|e| {
            error!("Error processing call: {:?}", e);
            RpcError::InternalError
        })?;

    return Ok(resp);
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
        .with_method(eth::BlockNumber, block_number)
        .with_method(eth::GetBalance, get_balance)
        .with_method(eth::Call, call)
        .finish();
    let plugin = Arc::new(plugin);

    block_on(async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    });
}

pub fn create_alloy_provider(
    transport: Arc<JsonRpcTransport>,
    url: String,
) -> impl alloy::providers::Provider {
    let host_transport = HostTransportService::new(transport, url);
    let client = RpcClient::new(host_transport, false);
    let provider = ProviderBuilder::new().connect_client(client);
    provider
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
