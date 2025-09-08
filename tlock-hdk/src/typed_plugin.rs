use tlock_pdk::{
    api::{PluginApi, PluginNamespace, TlockNamespace, methods::Methods},
    async_trait::async_trait,
    rpc_message::RpcErrorCode,
};

use crate::plugin::{Plugin, PluginError};

/// TypedPlugin is a type-safe wrapped plugin
pub struct TypedPlugin {
    plugin: Plugin,
}

impl TypedPlugin {
    pub fn new(plugin: Plugin) -> Self {
        Self { plugin }
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
impl TlockNamespace<PluginError> for TypedPlugin {
    async fn ping(&self, value: String) -> Result<String, PluginError> {
        rpc_call!(self, Methods::TlockPing, value)
    }
}
