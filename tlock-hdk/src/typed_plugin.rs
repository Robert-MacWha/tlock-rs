use std::sync::Arc;

use async_trait::async_trait;
use tlock_api::{
    PluginApi, methods::Methods, namespace_global::GlobalNamespace,
    namespace_plugin::PluginNamespace,
};
use wasmi_hdk::{
    plugin::{Plugin, PluginError},
    wasmi_pdk::{api::RequestHandler, rpc_message::RpcErrorCode},
};

/// TypedPlugin is a type-safe wrapped plugin
pub struct TypedPlugin {
    plugin: Plugin,
}

impl TypedPlugin {
    pub fn new(
        name: &str,
        wasm_bytes: Vec<u8>,
        handler: Arc<dyn RequestHandler<RpcErrorCode>>,
    ) -> Result<Self, wasmi::Error> {
        let plugin = Plugin::new(name, wasm_bytes, handler)?;
        Ok(Self { plugin })
    }
}

macro_rules! rpc_call {
    ($self:expr, $method:expr) => {{
        let res = $self.plugin.call(&$method.to_string(), serde_json::Value::Null).await?;
        serde_json::from_value(res.result).map_err(|_| RpcErrorCode::ParseError.into())
    }};
    ($self:expr, $method:expr, $( $arg:expr ),+) => {{
        let value = serde_json::to_value(($( $arg ),+)).map_err(|_| RpcErrorCode::ParseError)?;
        let res = $self.plugin.call(&$method.to_string(), value).await?;
        serde_json::from_value(res.result).map_err(|_| RpcErrorCode::ParseError.into())
    }};
}

impl PluginApi<PluginError> for TypedPlugin {}

#[async_trait]
impl PluginNamespace<PluginError> for TypedPlugin {
    async fn name(&self) -> Result<String, PluginError> {
        rpc_call!(self, Methods::PluginName)
    }

    async fn version(&self) -> Result<String, PluginError> {
        rpc_call!(self, Methods::PluginVersion)
    }
}

#[async_trait]
impl GlobalNamespace<PluginError> for TypedPlugin {
    async fn ping(&self, value: String) -> Result<String, PluginError> {
        rpc_call!(self, Methods::TlockPing, value)
    }
}
