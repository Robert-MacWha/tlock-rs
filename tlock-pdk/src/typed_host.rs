use std::sync::atomic::AtomicU64;

use crate::{
    api::{HostApi, TlockNamespace, methods::Methods},
    rpc_message::RpcErrorCode,
    transport::json_rpc_transport::JsonRpcTransport,
};

pub struct TypedHost<'a> {
    id: AtomicU64,
    transport: &'a JsonRpcTransport,
}

impl<'a> TypedHost<'a> {
    pub fn new(transport: &'a JsonRpcTransport) -> Self {
        Self {
            id: AtomicU64::new(0),
            transport,
        }
    }
}

impl HostApi<RpcErrorCode> for TypedHost<'_> {}

impl TlockNamespace<RpcErrorCode> for TypedHost<'_> {
    fn ping(&self, value: String) -> Result<String, RpcErrorCode> {
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let value = serde_json::to_value(value).map_err(|_| RpcErrorCode::ParseError)?;
        let resp = self
            .transport
            .call(id, &Methods::TlockPing.to_string(), value, None)?;
        let resp = serde_json::from_value(resp.result).map_err(|_| RpcErrorCode::ParseError)?;
        Ok(resp)
    }
}
