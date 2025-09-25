use std::sync::Arc;

use tlock_pdk::{
    async_trait::async_trait,
    register_plugin,
    tlock_api::{
        CompositeClient,
        plugin::{self, PluginNamespace},
        tlock::{self, TlockNamespace},
    },
    wasmi_pdk::rpc_message::RpcErrorCode,
};

struct MyPlugin {
    host: Arc<CompositeClient<RpcErrorCode>>,
}

impl MyPlugin {
    pub fn new(host: Arc<CompositeClient<RpcErrorCode>>) -> Self {
        Self { host }
    }
}

#[async_trait]
impl PluginNamespace for MyPlugin {
    type Error = RpcErrorCode;

    async fn name(&self) -> Result<String, Self::Error> {
        Ok("Test Async Plugin".to_string())
    }

    async fn version(&self) -> Result<String, Self::Error> {
        Ok("1.0.0".to_string())
    }
}

#[async_trait]
impl TlockNamespace for MyPlugin {
    type Error = RpcErrorCode;

    async fn ping(&self, message: String) -> Result<String, Self::Error> {
        Ok(format!("Pong: {}", message))
    }
}

register_plugin!(
    MyPlugin,
    [
        plugin::PluginNamespaceServer::new,
        tlock::TlockNamespaceServer::new
    ]
);
