use std::{sync::Arc, thread::sleep, time::Duration};

use log::{info, trace};
use tlock_hdk::{
    async_trait::async_trait,
    tlock_api::{Host, HostApi, namespace_global::GlobalNamespace},
    typed_plugin::TypedPlugin,
    wasmi_hdk::wasmi_pdk::rpc_message::RpcErrorCode,
};

//? current_thread uses single-threaded mode, simulating the browser environment
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .with_colors(true)
        .init()
        .ok();

    info!("Running single-threaded");
    let wasm_path = "../target/wasm32-wasip1/debug/rust-plugin-template.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;
    info!("Read {} kb from {}", wasm_bytes.len() / 1024, wasm_path);

    let handler = HostHandler {};
    let handler = Host(handler);
    let handler = Arc::new(handler);
    let plugin = TypedPlugin::new("Test Plugin", wasm_bytes, handler)?;

    let resp = plugin.ping("Hello Plugin!".into()).await?;
    info!("Received message: {:?}", resp);

    sleep(Duration::from_millis(1000));

    Ok(())
}

struct HostHandler {}

impl HostApi<RpcErrorCode> for HostHandler {}

#[async_trait]
impl GlobalNamespace<RpcErrorCode> for HostHandler {
    async fn ping(&self, message: String) -> Result<String, RpcErrorCode> {
        trace!("Host received ping with message: {}", message);
        Ok(format!("Host Pong: {}", message))
    }
}
