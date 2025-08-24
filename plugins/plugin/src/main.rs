use std::io::{self, BufReader};

use tlock_pdk::{
    api::TlockApi, json_rpc_transport::JsonRpcTransport, plugin_handler::PluginHandler,
};
struct Plugin<'a> {
    transport: &'a JsonRpcTransport,
}

fn main() {
    let writer = io::stdout();
    let reader = io::stdin();
    let reader = BufReader::new(reader);

    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));

    let plugin = Plugin {
        transport: &transport,
    };

    transport.process_next_line(Some(&plugin)).unwrap();
}

impl PluginHandler for Plugin<'_> {}

impl TlockApi for Plugin<'_> {
    fn ping(&self, message: &str) -> String {
        let version = self
            .transport
            .call(0, "tlock_version", "".into(), None)
            .unwrap();
        format!("Pong: version={:?} message={}", version, message)
    }

    fn version(&self) -> String {
        "1.0.0".to_string()
    }
}
