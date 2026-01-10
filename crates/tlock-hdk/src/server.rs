use serde_json::Value;
use tlock_api::{RpcMethod, alloy::transports::BoxFuture};
use wasmi_plugin_hdk::{host_handler::HostHandler, instance_id::InstanceId};
use wasmi_plugin_pdk::{router::MaybeSend, rpc_message::RpcError};

/// Lightweight HostServer wrapper that provides a typed interface for
/// registering RPC methods from tlock_api.
pub struct HostServer<S: Clone + Send + Sync + 'static> {
    inner: wasmi_plugin_hdk::server::HostServer<S>,
}

impl<S: Default + Clone + Send + Sync + 'static> Default for HostServer<S> {
    fn default() -> Self {
        Self {
            inner: wasmi_plugin_hdk::server::HostServer::default(),
        }
    }
}

impl<S: Clone + Send + Sync + 'static> HostServer<S> {
    pub fn new(state: S) -> Self {
        Self {
            inner: wasmi_plugin_hdk::server::HostServer::new(state),
        }
    }

    pub fn with_method<M, F, Fut>(mut self, _: M, func: F) -> Self
    where
        M: RpcMethod + 'static,
        F: Fn((InstanceId, S), M::Params) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<M::Output, RpcError>> + MaybeSend + 'static,
    {
        self.inner = self.inner.with_method(M::NAME, func);
        self
    }
}

impl<S: Clone + Send + Sync + 'static> HostHandler for HostServer<S> {
    fn handle<'a>(
        &'a self,
        instance: InstanceId,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, RpcError>> {
        self.inner.handle(instance, method, params)
    }
}
