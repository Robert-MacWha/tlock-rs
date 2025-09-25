use crate::methods::Methods;
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use wasmi_pdk::api::ApiError;

/// Plugin namespace, implemented by all plugins.
#[rpc_namespace]
#[async_trait]
pub trait PluginNamespace: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::PluginName)]
    /// Returns the name of the plugin.
    async fn name(&self) -> Result<String, Self::Error>;

    #[rpc_method(Methods::PluginVersion)]
    /// Returns the version of the plugin.
    async fn version(&self) -> Result<String, Self::Error>;
}
