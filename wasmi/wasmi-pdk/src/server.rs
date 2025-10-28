use std::{collections::HashMap, sync::Arc};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tracing::warn;

use crate::{api::RequestHandler, rpc_message::RpcError};

#[cfg(target_arch = "wasm32")]
pub type BoxFuture<'a, T> = futures::future::LocalBoxFuture<'a, T>;

#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<'a, T> = futures::future::BoxFuture<'a, T>;

#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T> MaybeSend for T {}

#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> MaybeSend for T {}

type HandlerFn<S> =
    Arc<dyn Send + Sync + Fn(Arc<S>, Value) -> BoxFuture<'static, Result<Value, RpcError>>>;

pub struct ServerBuilder<S> {
    state: Arc<S>,
    handlers: HashMap<String, HandlerFn<S>>,
}

/// Server is a RPC server that can handle requests by dispatching them to registered
/// handler functions. It stores a shared state `S` that is passed into each handler.
pub struct Server<S> {
    state: Arc<S>,
    handlers: HashMap<String, HandlerFn<S>>,
}

impl<S: Default + Send + Sync + 'static> Default for ServerBuilder<S> {
    fn default() -> Self {
        Self::new(Arc::new(S::default()))
    }
}

impl<S: Send + Sync + 'static> ServerBuilder<S> {
    pub fn new(state: Arc<S>) -> Self {
        Self {
            state,
            handlers: HashMap::new(),
        }
    }

    /// Register a new RPC method with the server. The method is identified by the
    /// given name, and the handler function should accept the shared state and
    /// deserialized params.
    ///
    /// Handlers should implement: `async fn handler(state: Arc<S>, params: P) -> Result<R, RpcError>`
    pub fn with_method<P, R, F, Fut>(mut self, name: &str, func: F) -> Self
    where
        P: DeserializeOwned + 'static,
        R: Serialize + 'static,
        F: Fn(Arc<S>, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R, RpcError>> + MaybeSend + 'static,
    {
        // Handler function that parses json params, calls the provided func,
        // and serializes the result back to json.
        let f = Arc::new(
            move |state: Arc<S>, params: Value| -> BoxFuture<'static, Result<Value, RpcError>> {
                let parsed = serde_json::from_value(params);
                let Ok(p) = parsed else {
                    return Box::pin(async move { Err(RpcError::InvalidParams) });
                };

                let fut = func(state, p);
                Box::pin(async move {
                    let result = fut.await?;
                    Ok(serde_json::to_value(result).unwrap())
                })
            },
        );

        self.handlers.insert(name.to_string(), f);
        return self;
    }

    pub fn finish(self) -> Server<S> {
        Server {
            state: self.state,
            handlers: self.handlers,
        }
    }
}

impl<S: Send + Sync + 'static> Server<S> {
    pub fn new(state: Arc<S>) -> ServerBuilder<S> {
        ServerBuilder::new(state)
    }

    pub async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        self.handle_with_state(self.state.clone(), method, params)
            .await
    }

    pub async fn handle_with_state(
        &self,
        state: Arc<S>,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcError> {
        let Some(handler) = self.handlers.get(method) else {
            warn!("Method not found: {}", method);
            return Err(RpcError::MethodNotFound);
        };
        handler(state, params).await
    }

    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S: Send + Sync + 'static> RequestHandler<RpcError> for Server<S> {
    fn handle<'a>(
        &'a self,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, RpcError>> {
        Box::pin(async move { self.handle(method, params).await })
    }
}
