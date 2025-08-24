use std::io::{self, BufReader};

use tlock_pdk::{
    api::TlockApi, json_rpc_transport::JsonRpcTransport, plugin_handler::PluginHandler,
};
struct Plugin {}

fn main() {
    let plugin = Plugin {};
    let transport = JsonRpcTransport::new();

    let reader = io::stdin();
    let mut reader = BufReader::new(reader);
    let mut writer = io::stdout();
    loop {
        transport
            .process_next_line(&mut reader, &mut writer, &plugin)
            .unwrap();
    }
}

impl PluginHandler for Plugin {}

impl TlockApi for Plugin {
    fn ping(&self, message: &str) -> String {
        format!("Pong: {}", message)
    }

    fn version(&self) -> String {
        "1.0.0".to_string()
    }
}
