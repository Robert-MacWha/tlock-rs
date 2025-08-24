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
