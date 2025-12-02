use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum RpcMessage {
    RpcRequest(RpcRequest),
    RpcResponse(RpcResponse),
    RpcErrorResponse(RpcErrorResponse),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RpcErrorResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub error: RpcError,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    #[error("Parse error")]
    ParseError,
    #[error("Invalid request")]
    InvalidRequest,
    #[error("Method not found")]
    MethodNotFound,
    #[error("Invalid params")]
    InvalidParams,
    #[error("Internal error")]
    InternalError,
    #[error("{0}")]
    Custom(String),
}

impl RpcError {
    pub fn code(&self) -> i64 {
        match self {
            RpcError::ParseError => -32700,
            RpcError::InvalidRequest => -32600,
            RpcError::MethodNotFound => -32601,
            RpcError::InvalidParams => -32602,
            RpcError::InternalError => -32603,
            RpcError::Custom(_) => -32000,
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }
}

pub fn to_rpc_err<E: std::error::Error>(e: E) -> RpcError {
    RpcError::Custom(e.to_string())
}

impl Serialize for RpcError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("RpcError", 2)?;
        state.serialize_field("code", &self.code())?;
        state.serialize_field("message", &self.message())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for RpcError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RpcErrorData {
            code: i64,
            message: String,
        }

        let data = RpcErrorData::deserialize(deserializer)?;

        Ok(match data.code {
            -32700 => RpcError::ParseError,
            -32600 => RpcError::InvalidRequest,
            -32601 => RpcError::MethodNotFound,
            -32602 => RpcError::InvalidParams,
            -32603 => RpcError::InternalError,
            _ => RpcError::Custom(data.message),
        })
    }
}
