use std::{io::stderr, sync::Arc, task::Poll};

use alloy::{
    providers::{Provider, ProviderBuilder},
    rpc::client::RpcClient,
    transports::{TransportError, TransportErrorKind, TransportFut, TransportResult},
};
use alloy_json_rpc::{RequestPacket, ResponsePacket};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    async_trait::async_trait,
    dispatcher::{Dispatcher, RpcHandler},
    futures::executor::block_on,
    state::get_state,
    tlock_api::{RpcMethod, eth, global, host},
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

#[derive(Serialize, Deserialize)]
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

#[async_trait]
impl RpcHandler<global::Ping> for EthProvider {
    async fn invoke(&self, _params: ()) -> Result<String, RpcErrorCode> {
        global::Ping.call(self.transport.clone(), ()).await?;
        Ok("pong".to_string())
    }
}

#[async_trait]
impl RpcHandler<eth::BlockNumber> for EthProvider {
    async fn invoke(&self, _params: ()) -> Result<u64, RpcErrorCode> {
        let state: ProviderState = get_state(self.transport.clone()).await?;

        let transport = HostTransportService::new(self.transport.clone(), state.rpc_url.clone());
        let client = RpcClient::new(transport, false);
        let provider = ProviderBuilder::new().connect_client(client);
        let block_number = provider.get_block_number().await.map_err(|e| {
            error!("Error fetching block number: {:?}", e);
            RpcErrorCode::InternalError
        })?;
        return Ok(block_number);
    }
}

fn main() {
    fmt().with_writer(stderr).init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let transport = Arc::new(transport);

    let plugin = EthProvider::new(transport.clone());
    let plugin = Arc::new(plugin);

    let mut dispatcher = Dispatcher::new(plugin);
    dispatcher.register::<global::Ping>();
    dispatcher.register::<eth::BlockNumber>();
    // dispatcher.register::<eth::GetBalance>();
    // dispatcher.register::<eth::Call>();
    let dispatcher = Arc::new(dispatcher);

    block_on(async move {
        let _ = transport.process_next_line(Some(dispatcher)).await;
    });
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

    async fn do_request(self, req: RequestPacket) -> TransportResult<ResponsePacket> {
        let params = host::Request {
            url: self.rpc_url,
            method: "POST".to_string(),
            headers: req
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.as_bytes().into()))
                .collect(),
            body: Some(serde_json::to_vec(&req).map_err(TransportErrorKind::custom)?),
        };

        let resp = host::Fetch
            .call(self.transport, params)
            .await
            .map_err(TransportErrorKind::custom)?;

        let Ok(body) = resp else {
            return Err(TransportErrorKind::custom_str(&resp.err().unwrap()));
        };

        return serde_json::from_slice(&body)
            .map_err(|err| TransportError::deser_err(err, String::from_utf8_lossy(&body)));
    }
}

impl Service<RequestPacket> for HostTransportService {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        let this = self.clone();
        Box::pin(this.do_request(req))
    }

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
