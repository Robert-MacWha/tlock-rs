use serde_json::Value;
pub use tlock_api;
pub use tlock_pdk;
pub use tracing;
pub use wasmi_plugin_hdk;
use wasmi_plugin_hdk::{host_handler::HostHandler, plugin::PluginId};
pub use wasmi_plugin_pdk;

use tlock_api::RpcMethod;
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

#[macro_export]
macro_rules! __impl_host_rpc_base {
    ($host_ty:ty, $method:ty, $host_fn:ident, $call_expr:expr) => {
        pub async fn $host_fn(
            host: (
                $crate::wasmi_plugin_hdk::plugin::PluginId,
                ::std::sync::Weak<$host_ty>,
            ),
            params: <$method as $crate::tlock_api::RpcMethod>::Params,
        ) -> Result<
            <$method as $crate::tlock_api::RpcMethod>::Output,
            $crate::wasmi_plugin_pdk::rpc_message::RpcError,
        > {
            use $crate::tracing::{info, warn};

            let plugin_id = &host.0;
            let host = host.1.upgrade().ok_or_else(|| {
                warn!("Host has been dropped");
                $crate::wasmi_plugin_pdk::rpc_message::RpcError::InternalError
            })?;

            info!("[host_func] Plugin {} sent {}", plugin_id, <$method>::NAME);
            $call_expr(host, plugin_id.clone(), params).await
        }
    };
}

#[macro_export]
macro_rules! impl_host_rpc {
    ($host_ty:ty, $method:ty, $host_fn:ident) => {
        $crate::__impl_host_rpc_base!(
            $host_ty,
            $method,
            $host_fn,
            |host: ::std::sync::Arc<$host_ty>,
             plugin_id: $crate::wasmi_plugin_hdk::plugin::PluginId,
             params: <$method as $crate::tlock_api::RpcMethod>::Params| async move {
                host.$host_fn(&plugin_id, params).await
            }
        );
    };
}

#[macro_export]
macro_rules! impl_host_rpc_no_id {
    ($host_ty:ty, $method:ty, $host_fn:ident) => {
        $crate::__impl_host_rpc_base!(
            $host_ty,
            $method,
            $host_fn,
            |host: ::std::sync::Arc<$host_ty>,
             _plugin_id: $crate::wasmi_plugin_hdk::plugin::PluginId,
             params: <$method as $crate::tlock_api::RpcMethod>::Params| async move {
                host.log_event(format!("Plugin {} called {}", _plugin_id, <$method>::NAME));
                host.$host_fn(params).await
            }
        );
    };
}
