use serde_json::Value;
use tlock_api::RpcMethod;
use wasmi_plugin_hdk::{host_handler::HostHandler, plugin::PluginId};
use wasmi_plugin_pdk::{
    rpc_message::RpcError,
    server::{BoxFuture, MaybeSend},
};

/// Lightweight wrapper around wasmi_plugin_pdk::server::PluginServer that provides
/// a typed interface for registering RPC methods from tlock_api.
pub struct HostServer<S: Clone + Send + Sync + 'static> {
    s: wasmi_plugin_hdk::server::HostServer<S>,
}

impl<S: Default + Clone + Send + Sync + 'static> Default for HostServer<S> {
    fn default() -> Self {
        Self {
            s: wasmi_plugin_hdk::server::HostServer::default(),
        }
    }
}

impl<S: Clone + Send + Sync + 'static> HostServer<S> {
    pub fn new(state: S) -> Self {
        Self {
            s: wasmi_plugin_hdk::server::HostServer::new(state),
        }
    }

    pub fn with_method<M, F, Fut>(mut self, _: M, func: F) -> Self
    where
        M: RpcMethod + 'static,
        F: Fn((PluginId, S), M::Params) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<M::Output, RpcError>> + MaybeSend + 'static,
    {
        self.s = self.s.with_method(M::NAME, func);
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
        self.s.handle(plugin, method, params)
    }
}
