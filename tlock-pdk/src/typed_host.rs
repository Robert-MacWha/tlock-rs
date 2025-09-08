use std::sync::{Arc, atomic::AtomicU64};

use async_trait::async_trait;

use crate::{
    api::{HostApi, TlockNamespace, methods::Methods},
    rpc_message::RpcErrorCode,
    transport::JsonRpcTransport,
};

pub struct TypedHost {
    id: AtomicU64,
    transport: Arc<JsonRpcTransport>,
}

impl TypedHost {
    pub fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self {
            id: AtomicU64::new(0),
            transport,
        }
    }
}

impl HostApi<RpcErrorCode> for TypedHost {}

#[async_trait]
impl TlockNamespace<RpcErrorCode> for TypedHost {
    async fn ping(&self, value: String) -> Result<String, RpcErrorCode> {
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let value = serde_json::to_value(value).map_err(|_| RpcErrorCode::ParseError)?;
        let resp = self
            .transport
            .call(id, &Methods::TlockPing.to_string(), value, None)
            .await?;
        let resp = serde_json::from_value(resp.result).map_err(|_| RpcErrorCode::ParseError)?;
        Ok(resp)
    }
}
