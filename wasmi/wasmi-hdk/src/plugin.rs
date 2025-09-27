use std::{io::BufReader, sync::Arc};

use futures::{AsyncBufReadExt, FutureExt};
use log::info;
use serde_json::Value;
use thiserror::Error;
use wasmi_pdk::{
    api::RequestHandler,
    async_trait::async_trait,
    rpc_message::{RpcError, RpcErrorCode, RpcResponse},
    transport::{JsonRpcTransport, Transport},
};

use crate::{
    compiled_plugin::CompiledPlugin,
    host_handler::HostHandler,
    plugin_instance::{PluginInstance, SpawnError},
};

pub type PluginId = String;

/// Plugin is an async-capable instance of a plugin
pub struct Plugin {
    name: String,
    id: PluginId,
    handler: Arc<dyn HostHandler>,
    compiled: CompiledPlugin,
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
        id: PluginId,
        wasm_bytes: Vec<u8>,
        handler: Arc<dyn HostHandler>,
    ) -> Result<Self, wasmi::Error> {
        let compiled = CompiledPlugin::new(wasm_bytes.clone())?;

        Ok(Plugin {
            name: name.to_string(),
            id,
            handler,
            compiled,
        })
    }
}

#[async_trait]
impl Transport<PluginError> for Plugin {
    async fn call(&self, method: &str, params: Value) -> Result<RpcResponse, PluginError> {
        let (instance, stdin_writer, stdout_reader, stderr_reader) =
            PluginInstance::new(self.compiled.clone())?;

        let stderr_task = async move {
            let mut buf_reader = futures::io::BufReader::new(stderr_reader);
            let mut line = String::new();
            while buf_reader.read_line(&mut line).await.is_ok_and(|n| n > 0) {
                info!(target: "plugin", "[{}] {}", self.name, line.trim_end());
                line.clear();
            }
        }
        .fuse();

        let handler = PluginCallback {
            handler: self.handler.clone(),
            uuid: self.id.clone(),
        };
        let handler = Arc::new(handler);

        let buf_reader = BufReader::new(stdout_reader);
        let transport =
            JsonRpcTransport::with_handler(Box::new(buf_reader), Box::new(stdin_writer), handler);
        let rpc_task = transport.call(method, params).fuse();

        futures::pin_mut!(stderr_task, rpc_task);

        let (res, _) = futures::join!(rpc_task, stderr_task);
        let res = res?;

        instance.kill();

        Ok(res)
    }
}

struct PluginCallback {
    handler: Arc<dyn HostHandler>,
    uuid: String,
}

#[async_trait]
impl RequestHandler<RpcErrorCode> for PluginCallback {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcErrorCode> {
        self.handler.handle(self.uuid.clone(), method, params).await
    }
}
