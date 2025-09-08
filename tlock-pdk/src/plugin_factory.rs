use std::sync::Arc;

use crate::typed_host::TypedHost;

/// A factory trait for creating plugin instances that handle JSON-RPC requests.
pub trait PluginFactory {
    fn new(transport: Arc<TypedHost>) -> Self;
}

#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        fn main() {
            use std::io::{self, BufReader};
            use std::sync::Arc;
            use tlock_pdk::plugin_factory::PluginFactory;
            use tlock_pdk::tlock_api::Plugin;
            use tlock_pdk::wasmi_pdk::{futures::executor, log, stderrlog};

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

            let host = TypedHost::new(transport.clone());
            let host = Arc::new(host);

            let plugin = <$plugin_type>::new(host.clone());
            let plugin = Plugin(plugin);
            let plugin = Arc::new(plugin);

            let runtime_future = async move {
                let _ = transport.process_next_line(Some(plugin)).await;
            };

            executor::block_on(runtime_future);
        }
    };
}
