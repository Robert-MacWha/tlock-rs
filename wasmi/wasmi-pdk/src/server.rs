use std::{collections::HashMap, sync::Arc};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::runtime::Builder;
use tracing::warn;

use crate::{api::RequestHandler, rpc_message::RpcError, transport::JsonRpcTransport};

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

/// Server is a RPC server that can handle requests by dispatching them to registered
/// handler functions. It stores a shared state `S` that is passed into each handler.
pub struct PluginServer {
    transport: Arc<JsonRpcTransport>,
    router: Router<JsonRpcTransport>,
}

pub struct Router<S> {
    handlers: HashMap<String, HandlerFn<S>>,
}

impl PluginServer {
    pub fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self {
            transport,
            router: Router::new(),
        }
    }

    pub fn new_with_transport() -> Self {
        let reader = std::io::BufReader::new(std::io::stdin());
        let writer = std::io::stdout();
        let transport = JsonRpcTransport::new(reader, writer);
        let transport = Arc::new(transport);

        Self {
            transport,
            router: Router::new(),
        }
    }

    pub fn with_method<P, R, F, Fut>(mut self, name: &str, func: F) -> Self
    where
        P: DeserializeOwned + 'static,
        R: Serialize + 'static,
        F: Fn(Arc<JsonRpcTransport>, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R, RpcError>> + MaybeSend + 'static,
    {
        self.router = self.router.with_method(name, func);
        self
    }

    pub fn run(self) {
        let server = Arc::new(self);
        let transport = server.transport.clone();

        let rt = Builder::new_current_thread().enable_time().build().unwrap();
        let local = tokio::task::LocalSet::new();

        rt.block_on(local.run_until(async move {
            let _ = transport.process_next_line(Some(server)).await;
        }));
    }
}

impl RequestHandler<RpcError> for PluginServer {
    fn handle<'a>(
        &'a self,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, RpcError>> {
        let state = self.transport.clone();
        Box::pin(async move { self.router.handle_with_state(state, method, params).await })
    }
}

impl<S: Send + Sync + 'static> Router<S> {
    pub fn new() -> Router<S> {
        Router {
            handlers: HashMap::new(),
        }
    }

    /// Register a new RPC method with the router. The method is identified by the
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
        self
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
}
