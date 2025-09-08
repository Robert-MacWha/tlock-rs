use std::{
    io::BufReader,
    sync::{Arc, atomic::AtomicU64},
};

use futures::{AsyncBufReadExt, FutureExt, select};
use log::{info, trace};
use runtime::yield_now;
use serde_json::Value;
use thiserror::Error;
use wasmi_pdk::{
    api::RequestHandler,
    rpc_message::{RpcErrorCode, RpcResponse},
    transport::JsonRpcTransport,
};

use crate::plugin_instance::{PluginInstance, SpawnError};

/// Plugin is an async-capable instance of a plugin
pub struct Plugin {
    name: String,
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
    pub fn new(
        name: &str,
        wasm_bytes: Vec<u8>,
        handler: Arc<dyn RequestHandler<RpcErrorCode>>,
    ) -> Self {
        Plugin {
            name: name.to_string(),
            wasm_bytes,
            id: AtomicU64::new(0),
            handler,
        }
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<RpcResponse, PluginError> {
        let (instance, stdin_writer, stdout_reader, stderr_reader) =
            PluginInstance::new(self.wasm_bytes.clone())?;

        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let stderr_task = async move {
            let mut buf_reader = futures::io::BufReader::new(stderr_reader);
            let mut line = String::new();
            while buf_reader.read_line(&mut line).await.is_ok_and(|n| n > 0) {
                info!(target: "plugin", "[{}] {}", self.name, line.trim_end());
                line.clear();
            }
        };

        let buf_reader = BufReader::new(stdout_reader);
        let transport = JsonRpcTransport::new(Box::new(buf_reader), Box::new(stdin_writer));

        let rpc_task = transport.call(id, method, params, Some(self.handler.clone()));

        let res = select! {
            res = rpc_task.fuse() => res.map_err(Into::into),
            _ = stderr_task.fuse() => Err(PluginError::PluginDied),
        };

        instance.kill();

        res
    }
}
