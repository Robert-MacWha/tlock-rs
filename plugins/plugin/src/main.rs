use std::sync::Arc;

use tlock_pdk::{
    async_trait::async_trait,
    register_plugin,
    tlock_api::{
        CompositeClient,
        domains::tlock::{TlockDomain, TlockDomainServer},
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
impl TlockDomain for MyPlugin {
    type Error = RpcErrorCode;

    async fn ping(&self, message: String) -> Result<String, Self::Error> {
        Ok(format!("Pong: {}", message))
    }

    async fn name(&self) -> Result<String, Self::Error> {
        Ok("Test Async Plugin".to_string())
    }

    async fn version(&self) -> Result<String, Self::Error> {
        Ok("1.0.0".to_string())
    }
}

register_plugin!(MyPlugin, [TlockDomainServer::new]);
