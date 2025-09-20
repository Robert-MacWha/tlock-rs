use std::sync::Arc;

use crate::{api::RequestHandler, rpc_message::RpcErrorCode, transport::JsonRpcTransport};

/// A factory trait for creating plugin instances that handle JSON-RPC requests.
pub trait PluginFactory: RequestHandler<RpcErrorCode> {
    fn new(transport: Arc<JsonRpcTransport>) -> Self;
}

/// Macro to register a plugin implementing the `PluginFactory` trait.
/// This macro is designed to be used in the `main.rs` file of a plugin crate,
/// and sets up the necessary boilerplate to initialize and run the plugin.
///  
/// # Example
/// ```rust,ignore
/// struct MyPlugin {}
///
/// impl PluginFactory for MyPlugin {
///     fn new(_: Arc<JsonRpcTransport>) -> Self {
///         Self {}
///     }
/// }
///
/// #[async_trait]
/// impl RequestHandler<RpcErrorCode> for MyPlugin {
///     async fn handle(
///         &self,
///         method: &str,
///         _params: serde_json::Value,
///     ) -> Result<serde_json::Value, RpcErrorCode> {
///         match method {
///             "hello" => Ok(serde_json::json!({"message": "Hello from MyPlugin!"})),
///             _ => Err(RpcErrorCode::MethodNotFound),
///         }
///     }
/// }
///
/// register_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        fn main() {
            use std::io::{self, BufReader};
            use std::sync::Arc;
            use wasmi_pdk::{futures::executor, log, plugin_factory::PluginFactory, stderrlog};

            // This ensures at compile-time that $plugin_type implements PluginFactory
            fn assert_factory<T: PluginFactory>() {}
            let _ = assert_factory::<$plugin_type>;

            stderrlog::new()
                .verbosity(stderrlog::LogLevelNum::Trace)
                .init()
                .unwrap();

            log::info!("Starting plugin...");

            // Setup stdio transport
            let writer = io::stdout();
            let reader = io::stdin();
            let reader = BufReader::new(reader);
            let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
            let transport = Arc::new(transport);

            let plugin = <$plugin_type>::new(transport.clone());
            let plugin = Arc::new(plugin);

            let runtime_future = async move {
                let _ = transport.process_next_line(Some(plugin)).await;
            };

            executor::block_on(runtime_future);
        }
    };
}
