use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use async_trait::async_trait;
use serde_json::Value;
use tlock_api::RpcMethod;
use wasmi_hdk::{host_handler::HostHandler, plugin::PluginId};
use wasmi_pdk::{rpc_message::RpcErrorCode, tracing::warn};

/// RpcHandler trait can be implemented by a struct to handle RPC calls for a
/// specific method M.
///
/// Methods must be registered with a Dispatcher instance.
#[async_trait]
pub trait RpcHandler<M: RpcMethod>: Send + Sync {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: M::Params,
    ) -> Result<M::Output, RpcErrorCode>;
}

#[async_trait]
trait ErasedHandler<T>: Send + Sync {
    async fn dispatch(
        &self,
        target: &T,
        plugin_id: PluginId,
        params: Value,
    ) -> Result<Value, RpcErrorCode>;
}

struct HandlerImpl<M: RpcMethod>(std::marker::PhantomData<M>);

#[async_trait]
impl<T, M> ErasedHandler<T> for HandlerImpl<M>
where
    T: RpcHandler<M> + Send + Sync,
    M: RpcMethod + 'static,
{
    async fn dispatch(
        &self,
        target: &T,
        plugin_id: PluginId,
        params: Value,
    ) -> Result<Value, RpcErrorCode> {
        let parsed: M::Params =
            serde_json::from_value(params).map_err(|_| RpcErrorCode::InvalidParams)?;
        let output = target.invoke(plugin_id, parsed).await?;
        serde_json::to_value(output).map_err(|_| RpcErrorCode::InternalError)
    }
}

/// A dispatcher routes incoming RPC requests to the appropriate handler based on
/// the method name. Methods must be registered with the dispatcher, and then
/// the dispatcher can be used as a RequestHandler to direct incoming requests
/// to the correct typed handler.
///
/// Separate plugin and host dispatchers are needed
/// because the host requires added context (the PluginId) when invoking handlers.
pub struct Dispatcher<T: Send + Sync> {
    handlers: HashMap<&'static str, Box<dyn ErasedHandler<T>>>,
    target: Weak<T>,
}

impl<T: Send + Sync> Dispatcher<T> {
    pub fn new(target: Weak<T>) -> Self {
        Self {
            handlers: HashMap::new(),
            target,
        }
    }

    pub fn register<M: RpcMethod + 'static>(&mut self)
    where
        T: RpcHandler<M> + Send + Sync + 'static,
    {
        self.handlers.insert(
            M::NAME,
            Box::new(HandlerImpl::<M>(std::marker::PhantomData)),
        );
    }

    pub async fn dispatch(
        &self,
        plugin_id: PluginId,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcErrorCode> {
        let target = self.target.upgrade().ok_or_else(|| {
            warn!("Dispatcher target has been dropped");
            RpcErrorCode::InternalError
        })?;

        match self.handlers.get(method) {
            Some(handler) => handler.dispatch(&target, plugin_id, params).await,
            None => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

#[async_trait]
impl<T: Send + Sync> HostHandler for Dispatcher<T> {
    async fn handle(
        &self,
        plugin: PluginId,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcErrorCode> {
        self.dispatch(plugin, method, params).await
    }
}
