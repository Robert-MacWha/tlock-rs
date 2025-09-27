use crate::methods::Methods;
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use wasmi_pdk::api::ApiError;

/// Global domain methods, implemented universally by all hosts and plugins.
#[rpc_namespace]
#[async_trait]
pub trait TlockDomain: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::PluginName)]
    /// Returns the name of the plugin.
    async fn name(&self) -> Result<String, Self::Error>;

    #[rpc_method(Methods::PluginVersion)]
    /// Returns the version of the plugin.
    async fn version(&self) -> Result<String, Self::Error>;

    #[rpc_method(Methods::TlockPing)]
    async fn ping(&self, msg: String) -> Result<String, Self::Error>;
}
