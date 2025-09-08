use std::sync::Arc;

use crate::{api::RequestHandler, rpc_message::RpcErrorCode, transport::JsonRpcTransport};

pub trait PluginFactory: RequestHandler<RpcErrorCode> {
    fn new(transport: Arc<JsonRpcTransport>) -> Self;
}

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
