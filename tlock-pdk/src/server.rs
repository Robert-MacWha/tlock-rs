use std::sync::Arc;

use tlock_api::RpcMethod;
use wasmi_plugin_pdk::{rpc_message::RpcError, server::MaybeSend, transport::JsonRpcTransport};

/// Lightweight wrapper around wasmi_plugin_pdk::server::PluginServer that provides
/// a typed interface for registering RPC methods from tlock_api.
pub struct PluginServer {
    s: wasmi_plugin_pdk::server::PluginServer,
}

impl PluginServer {
    pub fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self {
            s: wasmi_plugin_pdk::server::PluginServer::new(transport),
        }
    }

    pub fn new_with_transport() -> Self {
        Self {
            s: wasmi_plugin_pdk::server::PluginServer::new_with_transport(),
        }
    }

    pub fn with_method<M, F, Fut>(mut self, _: M, func: F) -> Self
    where
        M: RpcMethod + 'static,
        F: Fn(Arc<JsonRpcTransport>, M::Params) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<M::Output, RpcError>> + MaybeSend + 'static,
    {
        self.s = self.s.with_method(M::NAME, func);
        self
    }

    pub fn run(self) {
        self.s.run()
    }
}
