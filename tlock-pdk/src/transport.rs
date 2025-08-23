use std::sync::{Arc, mpsc::Receiver};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait RequestHandler: Send + Sync {
    fn handle(
        &self,
        method: &str,
        params: Value,
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
    Response {
        jsonrpc: String,
        id: u64,
        result: Option<Value>,
        error: Option<Value>,
    },
}

pub trait Transport {
    type Error: std::error::Error + Send + Sync + 'static;

    fn call(&mut self, method: &str, params: Value) -> Result<Receiver<RpcMessage>, Self::Error>;
    fn set_handler(&mut self, handler: Arc<dyn RequestHandler>);
}
