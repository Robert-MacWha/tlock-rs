use std::sync::Arc;

use tlock_api::RpcMethod;
use wasmi_pdk::{rpc_message::RpcError, server::MaybeSend};

/// ServerBuilder is a lightweight wrapper around wasmi_pdk::server::ServerBuilder
/// that provides an interface for registering typed RPC methods from the tlock_api.
pub struct ServerBuilder<S> {
    s: wasmi_pdk::server::ServerBuilder<S>,
}

impl<S: Default + Send + Sync + 'static> Default for ServerBuilder<S> {
    fn default() -> Self {
        Self::new(Arc::new(S::default()))
    }
}

impl<S: Send + Sync + 'static> ServerBuilder<S> {
    pub fn new(state: Arc<S>) -> Self {
        Self {
            s: wasmi_pdk::server::ServerBuilder::new(state),
        }
    }

    pub fn with_method<M, F, Fut>(mut self, _: M, func: F) -> Self
    where
        M: RpcMethod + 'static,
        F: Fn(Arc<S>, M::Params) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<M::Output, RpcError>> + MaybeSend + 'static,
    {
        self.s = self.s.with_method(M::NAME, func);
        self
    }

    pub fn finish(self) -> wasmi_pdk::server::Server<S> {
        self.s.finish()
    }
}
