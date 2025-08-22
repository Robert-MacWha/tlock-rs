use std::sync::{Arc, Mutex, mpsc::Sender};

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    },
}

pub struct PluginHandle<T: std::io::Write + Send> {
    pub id: u64,
    stdin: Arc<Mutex<T>>,
    tx: Sender<RpcMessage>,
}

pub struct Multiplexer {}
