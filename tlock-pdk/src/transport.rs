use std::{
    io::PipeReader,
    sync::mpsc::Receiver,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub type RequestHandler =
    dyn Fn(&str, Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> + Send + Sync;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum RpcMessage {
    Request {
        jsonrpc: String,
        id: u64,
        method: String,
        params: Value,
    },
    Response {
        jsonrpc: String,
        id: u64,
        result: Option<Value>,
        error: Option<Value>,
    },
}

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("transport error: {0}")]
    Generic(String),
}

pub trait Transport {
    type Error: std::error::Error + Send + Sync + 'static;

    fn call(&mut self, method: &str, params: Value) -> Result<Receiver<RpcMessage>, Self::Error>;
    fn set_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str, Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>
            + Send
            + Sync
            + 'static;
    fn start_polling(&mut self, stdout_reader: PipeReader) -> Result<(), Self::Error>;
}
