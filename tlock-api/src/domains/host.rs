use crate::{domains::Domains, methods::Methods};
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use wasmi_pdk::api::ApiError;

/// Host domain, implemented by the host environment providing services to plugins.
#[rpc_namespace]
#[async_trait]
pub trait HostDomain: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::PluginName)]
    /// Registers the calling plugin as the handler for a given entity.
    async fn register_entity(&self, domain: Domains, key: String) -> Result<(), Self::Error>;
}
