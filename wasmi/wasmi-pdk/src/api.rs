use async_trait::async_trait;
use serde_json::Value;

use crate::rpc_message::RpcErrorCode;

pub trait ApiError: From<RpcErrorCode> + Send + Sync {}
impl<T> ApiError for T where T: From<RpcErrorCode> + Send + Sync {}

/// JSON-RPC request handler.
///
/// The error type `E` must implement `From<RpcErrorCode>`, since the transport layer
/// may need to post errors of this type.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait RequestHandler<E: ApiError>: Send + Sync {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, E>;
}
