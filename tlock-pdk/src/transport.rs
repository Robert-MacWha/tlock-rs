use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub trait RequestHandler {
    fn handle(
        &mut self,
        method: &str,
        params: Value,
        transport: &mut dyn Transport,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum RpcMessage {
    Request {
        jsonrpc: String,
        id: u64,
        method: String,
        params: Value,
    },
    ResponseOk {
        jsonrpc: String,
        id: u64,
        result: Value,
    },
    ResponseErr {
        jsonrpc: String,
        id: u64,
        error: Value,
    },
}

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("serde error")]
    Serde(#[from] serde_json::Error),
    #[error("end of file")]
    EOF,
    #[error("response channel closed")]
    ChannelClosed,
    #[error("request timed out")]
    Timeout,
}

pub trait Transport {
    fn call(
        &mut self,
        method: &str,
        params: Value,
        handler: &mut dyn RequestHandler,
    ) -> Result<RpcMessage, TransportError>;
}
