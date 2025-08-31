use std::{sync::Arc, thread::sleep, time::Duration};

use tlock_hdk::{
    plugin::Plugin,
    tlock_pdk::{
        api::{Host, HostApi, TlockNamespace},
        async_trait::async_trait,
        rpc_message::RpcErrorCode,
    },
    typed_plugin::TypedPlugin,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_path = "../target/wasm32-wasip1/debug/rust-plugin-template.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;

    let handler = HostHandler {};
    let handler = Host(handler);
    let handler = Arc::new(handler);
    let plugin = Plugin::new(wasm_bytes, handler);
    let plugin = TypedPlugin::new(plugin);

    let resp = plugin.ping("Hello Plugin!".into()).await?;
    println!("Received message: {:?}", resp);

    sleep(Duration::from_millis(1000));

    Ok(())
}

struct HostHandler {}

impl HostApi<RpcErrorCode> for HostHandler {}

#[async_trait]
impl TlockNamespace<RpcErrorCode> for HostHandler {
    async fn ping(&self, message: String) -> Result<String, RpcErrorCode> {
        println!("Host received ping with message: {}", message);
        Ok(format!("Host Pong: {}", message))
    }
}
