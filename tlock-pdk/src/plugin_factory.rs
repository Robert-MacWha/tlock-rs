use std::sync::Arc;

use crate::typed_host::TypedHost;

pub trait PluginFactory {
    fn new(host: Arc<TypedHost>) -> Self;
}

#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        fn main() {
            use crate::PluginFactory;
            use std::io::{self, BufReader};
            use std::sync::Arc;
            use tlock_pdk::futures;
            use tlock_pdk::stderrlog;
            use tlock_pdk::{
                api::Plugin, transport::json_rpc_transport::JsonRpcTransport, typed_host::TypedHost,
            };

            // This ensures at compile-time that $plugin_type implements PluginFactory
            fn assert_factory<T: PluginFactory>() {}
            let _ = assert_factory::<$plugin_type>;

            stderrlog::new()
                .module(module_path!())
                .verbosity(stderrlog::LogLevelNum::Trace)
                .init()
                .unwrap();

            // Setup stdio transport
            let writer = io::stdout();
            let reader = io::stdin();
            let reader = BufReader::new(reader);
            let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
            let transport = Arc::new(transport);

            // Create host
            let host = TypedHost::new(transport.clone());
            let host = Arc::new(host);

            // Instantiate plugin
            let plugin_instance = <$plugin_type>::new(host.clone());
            let plugin = Plugin(plugin_instance);
            let plugin = Arc::new(plugin);

            // Process incoming request
            let runtime_future = async move {
                let _ = transport.process_next_line(Some(plugin)).await;
            };

            futures::executor::block_on(runtime_future);
        }
    };
}
