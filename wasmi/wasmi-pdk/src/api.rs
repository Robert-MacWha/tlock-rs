use async_trait::async_trait;
use serde_json::Value;

use crate::rpc_message::RpcErrorCode;

pub trait ApiError: From<RpcErrorCode> + Send + Sync {}
impl<T> ApiError for T where T: From<RpcErrorCode> + Send + Sync {}

#[async_trait]
pub trait RequestHandler<E: ApiError>: Send + Sync {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, E>;
}
