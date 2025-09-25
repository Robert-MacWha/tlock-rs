use crate::methods::Methods;
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use wasmi_pdk::api::ApiError;

/// Global namespace methods, implemented universally by all hosts and plugins.
#[rpc_namespace]
#[async_trait]
pub trait TlockNamespace: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::TlockPing)]
    async fn ping(&self, msg: String) -> Result<String, Self::Error>;
}
