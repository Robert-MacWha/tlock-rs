use std::sync::Arc;

use crate::typed_host::TypedHost;

pub trait PluginFactory {
    fn new(host: Arc<TypedHost>) -> Self;
}

// #[macro_export]
// macro_rules! register_plugin {
//     ($plugin_type:ty) => {
//         fn main() {
//             use crate::PluginFactory;
//             use std::io::{self, BufReader};
//             use tlock_pdk::{
//                 api::Plugin, transport::json_rpc_transport::JsonRpcTransport, typed_host::TypedHost,
//             };

//             // This ensures at compile-time that $plugin_type implements PluginFactory
//             fn assert_factory<T: PluginFactory>() {}
//             let _ = assert_factory::<$plugin_type>;

//             // Setup stdio transport
//             let writer = io::stdout();
//             let reader = io::stdin();
//             let reader = BufReader::new(reader);
//             let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));

//             // Create host
//             let host = TypedHost::new(&transport);

//             // Instantiate plugin
//             let plugin_instance:    $plugin_type = <$plugin_type>::new(&host);
//             let plugin = Plugin(plugin_instance);

//             // Process a single request
//             // transport.process_next_line(Some(&plugin)).await.unwrap();
//         }
//     };
// }
