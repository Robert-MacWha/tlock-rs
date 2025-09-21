// TODO: Clean this up and add docs. Would be nice to infer the namespace servers
// from the plugin's type (maybe a proc macro?) or at least to use the namespace
// rather than the `::new` constructor path. But I'm not skilled enough with
// macros to do that yet.
/// Registers a plugin's main function with the given namespace servers. The
/// plugin type must implement a `new(Arc<TypedHost>) -> Self` constructor.
///  
/// This macro automatically sets up the transport and logging over stdio and
/// reads the host's incoming request from stdin, dispatching the plugin.
#[macro_export]
macro_rules! register_plugin {
    // Explicit list of server constructors
    ($plugin_ty:ty, [ $($server_ctor:path),* $(,)? ]) => {
        fn main() {
            // Transport (stdio)
            let writer = ::std::io::stdout();
            let reader = ::std::io::BufReader::new(::std::io::stdin());
            let transport = ::tlock_pdk::wasmi_pdk::transport::JsonRpcTransport::new(
                Box::new(reader),
                Box::new(writer),
            );
            let transport = ::std::sync::Arc::new(transport);

            // Logging
            ::tlock_pdk::wasmi_pdk::stderrlog::new()
                .verbosity(::tlock_pdk::wasmi_pdk::stderrlog::LogLevelNum::Trace)
                .init()
                .unwrap();
            ::tlock_pdk::wasmi_pdk::log::info!("Starting plugin...");

            // Host + plugin
            let host = ::std::sync::Arc::new(::tlock_pdk::tlock_api::CompositeClient::new(transport.clone()));
            let plugin = ::std::sync::Arc::new(<$plugin_ty>::new(host.clone()));

            // Composite server
            let mut handler = ::tlock_pdk::tlock_api::CompositeServer::new();
            $(
                handler.register($server_ctor(plugin.clone()));
            )*
            let handler = ::std::sync::Arc::new(handler);

            // Run loop
            let fut = async move {
                let _ = transport.process_next_line(Some(handler)).await;
            };
            ::tlock_pdk::wasmi_pdk::futures::executor::block_on(fut);
        }
    };
}
