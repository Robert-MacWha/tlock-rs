use async_trait::async_trait;
use wasmi_pdk::api::ApiError;

/// Plugin namespace, implemented by all plugins.
#[async_trait]
pub trait PluginNamespace<E: ApiError>: Send + Sync {
    async fn name(&self) -> Result<String, E>;
    async fn version(&self) -> Result<String, E>;
}
