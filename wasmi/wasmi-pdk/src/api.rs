use serde_json::Value;

use crate::{rpc_message::RpcErrorCode, server::BoxFuture};

pub trait ApiError: From<RpcErrorCode> + Send + Sync {}
impl<T> ApiError for T where T: From<RpcErrorCode> + Send + Sync {}

/// JSON-RPC request handler.
///
/// The error type `E` must implement `From<RpcErrorCode>`, since the transport layer
/// may need to post errors of this type.
pub trait RequestHandler<E>: Send + Sync {
    fn handle<'a>(&'a self, method: &'a str, params: Value) -> BoxFuture<'a, Result<Value, E>>;
}
