use std::{io::stderr, sync::Arc, task::Poll};

use alloy::{
    providers::{Provider, ProviderBuilder},
    rpc::client::RpcClient,
    transports::{TransportError, TransportErrorKind, TransportFut},
};
use alloy_json_rpc::{RequestPacket, ResponsePacket};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    dispatcher::Dispatcher,
    futures::executor::block_on,
    impl_rpc_handler,
    state::{get_state, set_state},
    tlock_api::{
        RpcMethod,
        component::{container, text},
        entities::{EthProviderId, PageId},
        eth, global, host, page, plugin,
    },
    wasmi_pdk::{
        rpc_message::RpcErrorCode,
        tracing::{error, info},
        tracing_subscriber::fmt,
        transport::JsonRpcTransport,
    },
};
use tower_service::Service;

struct EthProvider {
    transport: Arc<JsonRpcTransport>,
}

#[derive(Serialize, Deserialize, Default)]
struct ProviderState {
    rpc_url: String,
}

impl EthProvider {
    pub fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self {
            transport: transport,
        }
    }
}

impl_rpc_handler!(EthProvider, global::Ping, |self, _params| {
    global::Ping.call(self.transport.clone(), ()).await?;
    Ok("pong".to_string())
});

impl_rpc_handler!(EthProvider, plugin::Init, |self, _params| {
    info!("Initializing Ethereum Provider Plugin...");
    let interface_id = PageId::new("eth_provider_page".to_string());
    host::RegisterEntity
        .call(self.transport.clone(), interface_id.into())
        .await?;

    info!("Registering Ethereum Provider...");

    let provider_id = EthProviderId::new("eth_provider".to_string());
    host::RegisterEntity
        .call(self.transport.clone(), provider_id.into())
        .await?;

    let state = ProviderState {
        rpc_url: "https://eth.llamarpc.com".to_string(),
    };
    set_state(self.transport.clone(), &state).await?;

    Ok(())
});

impl_rpc_handler!(EthProvider, page::OnLoad, |self, interface_id| {
    let state: ProviderState = get_state(self.transport.clone()).await.unwrap_or_default();

    let component = container(vec![
        text("This is the Ethereum Provider Plugin").into(),
        text(format!("RPC URL: {}", state.rpc_url)).into(),
    ]);

    host::SetInterface
        .call(self.transport.clone(), (interface_id, component))
        .await?;

    return Ok(());
});

impl_rpc_handler!(EthProvider, eth::BlockNumber, |self, _params| {
    let state: ProviderState = get_state(self.transport.clone()).await?;

    let provider = create_alloy_provider(self.transport.clone(), state.rpc_url);
    let block_number = provider.get_block_number().await.map_err(|e| {
        error!("Error fetching block number: {:?}", e);
        RpcErrorCode::InternalError
    })?;
    return Ok(block_number);
});

impl_rpc_handler!(EthProvider, eth::GetBalance, |self, params| {
    let state: ProviderState = get_state(self.transport.clone()).await?;
    let (address, block_id) = params;

    let provider = create_alloy_provider(self.transport.clone(), state.rpc_url);
    let balance = provider
        .get_balance(address)
        .block_id(block_id)
        .await
        .map_err(|e| {
            error!("Error fetching balance: {:?}", e);
            RpcErrorCode::InternalError
        })?;
    return Ok(balance);
});

impl_rpc_handler!(EthProvider, eth::Call, |self, params| {
    let state: ProviderState = get_state(self.transport.clone()).await?;

    let (tx, block_overrides, state_overrides) = params;

    let provider = create_alloy_provider(self.transport.clone(), state.rpc_url);
    let resp = provider
        .call(tx)
        .with_block_overrides_opt(block_overrides)
        .overrides_opt(state_overrides)
        .await
        .map_err(|e| {
            error!("Error processing call: {:?}", e);
            RpcErrorCode::InternalError
        })?;

    return Ok(resp);
});

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

    let plugin = EthProvider::new(transport.clone());
    let plugin = Arc::new(plugin);

    let mut dispatcher = Dispatcher::new(plugin);
    dispatcher.register::<global::Ping>();
    dispatcher.register::<plugin::Init>();
    dispatcher.register::<page::OnLoad>();
    dispatcher.register::<eth::BlockNumber>();
    dispatcher.register::<eth::GetBalance>();
    dispatcher.register::<eth::Call>();
    let dispatcher = Arc::new(dispatcher);

    block_on(async move {
        let _ = transport.process_next_line(Some(dispatcher)).await;
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
