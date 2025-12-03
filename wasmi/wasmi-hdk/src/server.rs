use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use wasmi_pdk::{
    rpc_message::RpcError,
    server::{BoxFuture, MaybeSend, Router},
};

use crate::{host_handler::HostHandler, plugin::PluginId};

pub struct HostServer<S: Clone + Send + Sync + 'static> {
    state: Arc<S>,
    router: Router<(PluginId, Arc<S>)>,
}

impl<S: Default + Clone + Send + Sync + 'static> Default for HostServer<S> {
    fn default() -> Self {
        Self {
            state: Arc::new(S::default()),
            router: Router::new(),
        }
    }
}

impl<S: Clone + Send + Sync + 'static> HostServer<S> {
    pub fn new(state: Arc<S>) -> Self {
        Self {
            router: Router::new(),
            state,
        }
    }

    pub fn with_method<P, R, F, Fut>(mut self, name: &str, func: F) -> Self
    where
        P: DeserializeOwned + 'static,
        R: Serialize + 'static,
        F: Fn(Arc<(PluginId, Arc<S>)>, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R, RpcError>> + MaybeSend + 'static,
    {
        self.router = self.router.with_method(name, func);
        self
    }
}

impl<S: Clone + Send + Sync + 'static> HostHandler for HostServer<S> {
    fn handle<'a>(
        &'a self,
        plugin: PluginId,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, RpcError>> {
        Box::pin(async move {
            let s = self.state.clone();
            let s = Arc::new((plugin, s));
            self.router.handle_with_state(s, method, params).await
        })
    }
}
