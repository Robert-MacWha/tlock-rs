use std::{thread::sleep, time::Duration};

use tlock_hdk::{
    plugin::Plugin,
    tlock_pdk::{
        api::{Host, HostApi, TlockNamespace},
        rpc_message::RpcErrorCode,
    },
    typed_plugin::TypedPlugin,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_path = "../target/wasm32-wasip1/debug/rust-plugin-template.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;

    let handler = HostHandler {};
    let handler = Host(handler);
    let plugin = Plugin::new(wasm_bytes, &handler);
    let plugin = TypedPlugin::new(plugin);

    let resp = plugin.ping("Hello Plugin!".into());
    println!("Received message: {:?}", resp);

    sleep(Duration::from_millis(1000));

    Ok(())
}

struct HostHandler {}

impl HostApi<RpcErrorCode> for HostHandler {}

impl TlockNamespace<RpcErrorCode> for HostHandler {
    fn ping(&self, message: String) -> Result<String, RpcErrorCode> {
        println!("Host received ping with message: {}", message);
        Ok(format!("Host Pong: {}", message))
    }
}
