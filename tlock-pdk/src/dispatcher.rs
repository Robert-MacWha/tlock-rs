use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;
use tlock_api::RpcMethod;
use wasmi_pdk::{
    api::RequestHandler,
    log::{info, warn},
    rpc_message::RpcErrorCode,
};

#[async_trait]
pub trait RpcHandler<M: RpcMethod>: Send + Sync {
    async fn invoke(&self, params: M::Params) -> Result<M::Output, RpcErrorCode>;
}

#[async_trait]
trait ErasedHandler<T>: Send + Sync {
    async fn dispatch(&self, target: &T, params: Value) -> Result<Value, RpcErrorCode>;
}

struct HandlerImpl<M: RpcMethod>(std::marker::PhantomData<M>);

#[async_trait]
impl<T, M> ErasedHandler<T> for HandlerImpl<M>
where
    T: RpcHandler<M> + Send + Sync,
    M: RpcMethod + 'static,
{
    async fn dispatch(&self, target: &T, params: Value) -> Result<Value, RpcErrorCode> {
        info!("Dispatching method: {}", M::NAME);
        let parsed: M::Params = serde_json::from_value(params.clone()).map_err(|_| {
            warn!(
                "Failed to parse params for method {}, {:?}",
                M::NAME,
                params
            );
            return RpcErrorCode::InvalidParams;
        })?;
        let output = target.invoke(parsed).await?;
        serde_json::to_value(output).map_err(|_| RpcErrorCode::InternalError)
    }
}

pub struct Dispatcher<T: Send + Sync> {
    handlers: HashMap<&'static str, Box<dyn ErasedHandler<T>>>,
    target: Arc<T>,
}

impl<T: Send + Sync> Dispatcher<T> {
    pub fn new(target: Arc<T>) -> Self {
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

    pub async fn dispatch(&self, method: &str, params: Value) -> Result<Value, RpcErrorCode> {
        match self.handlers.get(method) {
            Some(handler) => handler.dispatch(&self.target, params).await,
            None => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

#[async_trait]
impl<T: Send + Sync> RequestHandler<RpcErrorCode> for Dispatcher<T> {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcErrorCode> {
        self.dispatch(method, params).await
    }
}
