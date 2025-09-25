use std::sync::Arc;

use async_trait::async_trait;
use wasmi_pdk::{
    api::{ApiError, RequestHandler},
    log::warn,
    rpc_message::RpcErrorCode,
    transport::Transport,
};

use crate::{plugin::PluginNamespaceClient, tlock::TlockNamespaceClient};

pub mod caip;
pub mod eip155_keyring;
pub mod eip155_provider;
pub mod methods;
pub mod plugin;
pub mod tlock;
pub use alloy_dyn_abi;
pub use alloy_primitives;
pub use alloy_rpc_types;

pub struct CompositeClient<E: ApiError> {
    plugin: PluginNamespaceClient<E>,
    global: TlockNamespaceClient<E>,
}

impl<E: ApiError> CompositeClient<E> {
    pub fn new(transport: Arc<dyn Transport<E> + Send + Sync>) -> Self {
        Self {
            plugin: PluginNamespaceClient::new(transport.clone()),
            global: TlockNamespaceClient::new(transport),
        }
    }

    pub fn plugin(&self) -> &PluginNamespaceClient<E> {
        &self.plugin
    }

    pub fn global(&self) -> &TlockNamespaceClient<E> {
        &self.global
    }
}

pub struct CompositeServer<E: ApiError> {
    servers: Vec<Box<dyn RequestHandler<E> + Send + Sync>>,
}

impl<E: ApiError> CompositeServer<E> {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    pub fn register<H>(&mut self, handler: H)
    where
        H: RequestHandler<E> + Send + Sync + 'static,
    {
        self.servers.push(Box::new(handler));
    }
}

#[async_trait]
impl<E: ApiError> RequestHandler<E> for CompositeServer<E> {
    async fn handle(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, E> {
        for server in &self.servers {
            if let Ok(result) = server.handle(method, params.clone()).await {
                return Ok(result);
            }
        }

        warn!("Method handler not found: {}", method);
        Err(E::from(RpcErrorCode::MethodNotFound))
    }
}
