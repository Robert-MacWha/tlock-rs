use tlock_api::RpcMethod;
use wasmi_plugin_pdk::{router::MaybeSend, rpc_message::RpcError, transport::Transport};

/// Lightweight Runner wrapper that provides a typed interface for registering
/// RPC methods from tlock_api.
pub struct PluginRunner {
    inner: wasmi_plugin_pdk::runner::PluginRunner,
}

impl PluginRunner {
    pub fn new() -> Self {
        Self {
            inner: wasmi_plugin_pdk::runner::PluginRunner::new(),
        }
    }

    pub fn with_method<M, F, Fut>(mut self, _: M, func: F) -> Self
    where
        M: RpcMethod + 'static,
        F: Fn(Transport, M::Params) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<M::Output, RpcError>> + MaybeSend + 'static,
    {
        self.inner = self.inner.with_method(M::NAME, func);
        self
    }

    pub fn run(self) {
        self.inner.run()
    }
}
