use std::sync::Arc;

use async_trait::async_trait;
use wasmi_pdk::{
    api::{ApiError, RequestHandler},
    log::warn,
    rpc_message::RpcErrorCode,
    transport::Transport,
};

pub mod methods;
pub use alloy_dyn_abi;
pub use alloy_primitives;
pub use alloy_rpc_types;

use crate::domains::{
    eip155_keyring::Eip155KeyringClient, host::HostDomainClient, tlock::TlockDomainClient,
};
pub mod domains;
pub mod routes;

/// A composite client that provides typed access to all supported domains.
pub struct CompositeClient<E: ApiError> {
    tlock: TlockDomainClient<E>,
    host: HostDomainClient<E>,
    eip155_keyring: Eip155KeyringClient<E>,
    // eip155_provider: Eip155ProviderClient<E>,
}

impl<E: ApiError> CompositeClient<E> {
    pub fn new(transport: Arc<dyn Transport<E> + Send + Sync>) -> Self {
        Self {
            tlock: TlockDomainClient::new(transport.clone()),
            host: HostDomainClient::new(transport.clone()),
            eip155_keyring: Eip155KeyringClient::new(transport.clone()),
            // eip155_provider: Eip155ProviderClient::new(transport.clone()),
        }
    }

    pub fn tlock(&self) -> &TlockDomainClient<E> {
        &self.tlock
    }

    pub fn host(&self) -> &HostDomainClient<E> {
        &self.host
    }

    pub fn eip155_keyring(&self) -> &Eip155KeyringClient<E> {
        &self.eip155_keyring
    }

    // pub fn eip155_provider(&self) -> &Eip155ProviderClient<E> {
    //     &self.eip155_provider
    // }
}

/// A composite server that can route requests to any registered domain handler.
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
