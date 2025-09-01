use std::{
    io::{BufRead, BufReader},
    sync::{Arc, atomic::AtomicU64},
};

use futures::{FutureExt, select};
use runtime::yield_now;
use serde_json::Value;
use thiserror::Error;
use tlock_pdk::{
    api::RequestHandler,
    rpc_message::{RpcErrorCode, RpcResponse},
    transport::json_rpc_transport::JsonRpcTransport,
};

use crate::plugin_instance::{PluginInstance, SpawnError};

/// Plugin is an async-capable instance of a plugin
pub struct Plugin {
    wasm_bytes: Vec<u8>,
    id: AtomicU64,
    handler: Arc<dyn RequestHandler<RpcErrorCode>>,
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("spawn error")]
    SpawnError(#[from] SpawnError),
    #[error("transport error")]
    RpcError(#[from] RpcErrorCode),
    #[error("plugin died")]
    PluginDied,
}

impl Plugin {
    pub fn new(wasm_bytes: Vec<u8>, handler: Arc<dyn RequestHandler<RpcErrorCode>>) -> Self {
        Plugin {
            wasm_bytes,
            id: AtomicU64::new(0),
            handler,
        }
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<RpcResponse, PluginError> {
        let (instance, stdin_writer, stdout_reader) = PluginInstance::new(self.wasm_bytes.clone())?;

        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let buf_reader = BufReader::new(stdout_reader);
        let transport = JsonRpcTransport::new(Box::new(buf_reader), Box::new(stdin_writer));

        println!("Calling plugin method: {}", method);

        select! {
            res = transport.call(id, method, params, Some(self.handler.clone())).fuse() => {
                res.map_err(Into::into)
            }
            _ = async {
                loop {
                    if !instance.is_running() { break; }
                    yield_now().await;
                }
            }.fuse() => {
                Err(PluginError::PluginDied)
            }
        }
    }
}
