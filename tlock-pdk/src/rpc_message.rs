use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum RpcMessage {
    RpcRequest(RpcRequest),
    RpcResponse(RpcResponse),
    RpcErrorResponse(RpcErrorResponse),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcErrorResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub error: RpcError,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcError {
    pub code: RpcErrorCode,
    pub message: String,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i64)]
/// EIP-1474 error codes:
/// https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1474.md
pub enum RpcErrorCode {
    // standard
    #[error("Parse error")]
    ParseError = -32700,
    #[error("Invalid request")]
    InvalidRequest = -32600,
    #[error("Method not found")]
    MethodNotFound = -32601,
    #[error("Invalid params")]
    InvalidParams = -32602,
    #[error("Internal error")]
    InternalError = -32603,
    // non-standard
    #[error("Invalid input")]
    InvalidInput = -32000,
    #[error("Resource not found")]
    ResourceNotFound = -32001,
    #[error("Resource unavailable")]
    ResourceUnavailable = -32002,
    #[error("Transaction rejected")]
    TransactionRejected = -32003,
    #[error("Method not supported")]
    MethodNotSupported = -32004,
    #[error("Limit exceeded")]
    LimitExceeded = -32005,
    #[error("JSON-RPC version not supported")]
    JsonRpcVersionNotSupported = -32006,
}

impl RpcErrorResponse {
    pub fn from_rpc_error(id: u64, code: RpcErrorCode) -> Self {
        RpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            id,
            error: RpcError {
                code,
                message: code.to_string(),
            },
        }
    }
}
