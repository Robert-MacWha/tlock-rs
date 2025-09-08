use async_trait::async_trait;
use wasmi_pdk::api::ApiError;

/// Global namespace methods, implemented universally by all hosts and plugins.
#[async_trait]
pub trait GlobalNamespace<E: ApiError>: Send + Sync {
    async fn ping(&self, _msg: String) -> Result<String, E> {
        Ok("Pong".into())
    }
}
