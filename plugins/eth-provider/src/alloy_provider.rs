use alloy::{
    providers::ProviderBuilder,
    rpc::client::RpcClient,
    transports::{TransportError, TransportErrorKind, TransportFut},
};
use alloy_json_rpc::{RequestPacket, ResponsePacket};
use std::{sync::Arc, task::Poll};
use tlock_pdk::{
    tlock_api::{RpcMethod, host},
    wasmi_pdk::transport::JsonRpcTransport,
};
use tower_service::Service;

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
                return Err(TransportErrorKind::custom_str(
                    &resp.err().unwrap_or("Unknown Error".into()),
                ));
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
