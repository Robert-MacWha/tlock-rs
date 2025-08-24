use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::request_handler::RequestHandler;

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
        error: String,
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
    type Error;
    fn call(
        &self,
        reader: impl Read,
        writer: &mut dyn Write,
        method: &str,
        params: Value,
        handler: &dyn RequestHandler,
    ) -> Result<RpcMessage, Self::Error>;
}
