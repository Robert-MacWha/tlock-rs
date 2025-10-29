use std::{fmt::Display, io::BufReader, sync::Arc};

use futures::{AsyncBufReadExt, FutureExt};
use serde_json::Value;
use thiserror::Error;
use tracing::info;
use wasmi_pdk::{
    api::RequestHandler,
    async_trait::async_trait,
    rpc_message::{RpcError, RpcResponse},
    server::BoxFuture,
    transport::{JsonRpcTransport, Transport},
};

use crate::{
    compiled_plugin::CompiledPlugin,
    host_handler::HostHandler,
    plugin_instance::{PluginInstance, SpawnError},
};

/// TODO: Adjust this so it's a UUID and copyable
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginId(String);

impl From<PluginId> for String {
    fn from(plugin_id: PluginId) -> Self {
        plugin_id.0
    }
}

impl From<&str> for PluginId {
    fn from(s: &str) -> Self {
        PluginId(s.to_owned())
    }
}

impl From<String> for PluginId {
    fn from(s: String) -> Self {
        PluginId(s)
    }
}

impl Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

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
    RpcError(#[from] RpcError),
    #[error("plugin died")]
    PluginDied,
}

impl Plugin {
    pub fn new(
        name: &str,
        id: &PluginId,
        wasm_bytes: Vec<u8>,
        handler: Arc<dyn HostHandler>,
    ) -> Result<Self, wasmi::Error> {
        let compiled = CompiledPlugin::new(wasm_bytes.clone())?;

        Ok(Plugin {
            name: name.to_string(),
            id: id.clone(),
            handler,
            compiled,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> PluginId {
        self.id.clone()
    }
}

impl PluginError {
    pub fn as_rpc_code(&self) -> RpcError {
        match self {
            PluginError::RpcError(code) => code.clone(),
            _ => RpcError::InternalError,
        }
    }
}

impl From<PluginError> for RpcError {
    fn from(err: PluginError) -> Self {
        match err {
            PluginError::RpcError(code) => code,
            _ => RpcError::InternalError,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Transport<PluginError> for Plugin {
    async fn call(&self, method: &str, params: Value) -> Result<RpcResponse, PluginError> {
        let (instance, stdin_writer, stdout_reader, stderr_reader, instance_task) =
            PluginInstance::new(self.compiled.clone())?;

        let name = self.name.clone();
        let stderr_task = async move {
            let mut buf_reader = futures::io::BufReader::new(stderr_reader);
            let mut line = String::new();
            while buf_reader.read_line(&mut line).await.is_ok_and(|n| n > 0) {
                info!(target: "plugin", "[plugin] [{}] {}", name, line.trim_end());
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

        let instance_task = instance_task.fuse();
        futures::pin_mut!(rpc_task, instance_task, stderr_task);
        let (res, _, _) = futures::join!(rpc_task, instance_task, stderr_task);

        instance.kill();
        let res = res?;
        Ok(res)
    }
}

struct PluginCallback {
    handler: Arc<dyn HostHandler>,
    uuid: PluginId,
}

impl RequestHandler<RpcError> for PluginCallback {
    fn handle<'a>(
        &'a self,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, RpcError>> {
        Box::pin(async move { self.handler.handle(self.uuid.clone(), method, params).await })
    }
}
