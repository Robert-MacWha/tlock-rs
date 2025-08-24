// plugin.rs
use std::{
    io::{self, BufRead},
    sync::{Arc, Mutex},
};

use tlock_pdk::{
    api::TlockApi, host::Host, plugin_handler::PluginHandler, transport::RequestHandler,
};

struct Plugin {}

fn main() {
    let plugin = Plugin {};
    let host = Host::new(
        Box::new(io::stdin()),
        Box::new(io::stdout()),
        Some(Arc::new(Mutex::new(plugin))),
    )
    .unwrap();
}

impl PluginHandler for Plugin {}

impl TlockApi for Plugin {
    fn ping(&mut self, message: &str) -> String {
        format!("Pong: {}", message)
    }

    fn version(&mut self) -> String {
        "1.0.0".to_string()
    }
}
