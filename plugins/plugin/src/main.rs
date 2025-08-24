mod json_rpc_transport;
mod plugin_handler;
mod request_handler;
mod transport;
use std::io::{self, BufReader};

use crate::{
    json_rpc_transport::JsonRpcTransport,
    plugin_handler::{PluginHandler, TlockApi},
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
