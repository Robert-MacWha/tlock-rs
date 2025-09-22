use std::sync::Arc;

use async_trait::async_trait;
use wasmi_pdk::{
    api::{ApiError, RequestHandler},
    log::warn,
    rpc_message::RpcErrorCode,
    transport::Transport,
};

use crate::{global::GlobalNamespaceClient, plugin::PluginNamespaceClient};

pub mod caip;
pub mod eip155_keyring;
pub mod global;
pub mod methods;
pub mod namespace_eth;
pub mod plugin;

pub struct CompositeClient<E: ApiError> {
    plugin: PluginNamespaceClient<E>,
    global: GlobalNamespaceClient<E>,
}

impl<E: ApiError> CompositeClient<E> {
    pub fn new(transport: Arc<dyn Transport<E> + Send + Sync>) -> Self {
        Self {
            plugin: PluginNamespaceClient::new(transport.clone()),
            global: GlobalNamespaceClient::new(transport),
        }
    }

    pub fn plugin(&self) -> &PluginNamespaceClient<E> {
        &self.plugin
    }

    pub fn global(&self) -> &GlobalNamespaceClient<E> {
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

        warn!("Method not found: {}", method);
        Err(E::from(RpcErrorCode::MethodNotFound))
    }
}
