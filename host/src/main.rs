use log::{info, trace};
use std::{sync::Arc, thread::sleep, time::Duration};
use tlock_hdk::{
    async_trait::async_trait,
    wasmi_hdk::{
        host_handler::HostHandler,
        plugin::{Plugin, PluginId},
        wasmi_pdk::rpc_message::RpcErrorCode,
    },
};

struct Host {}

//? current_thread uses single-threaded mode, simulating the browser environment
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .with_colors(true)
        .init()
        .ok();

    info!("Running single-threaded");
    let wasm_path = "target/wasm32-wasip1/debug/rust-plugin-template.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;
    info!("Read {} kb from {}", wasm_bytes.len() / 1024, wasm_path);

    let host = Host {};
    let host = Arc::new(host);
    let plugin = Plugin::new("Test Plugin", "0001".into(), wasm_bytes, host.clone())?;

    sleep(Duration::from_millis(1000));

    Ok(())
}

#[async_trait]
impl HostHandler for Host {
    async fn handle(
        &self,
        plugin: PluginId,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, RpcErrorCode> {
        trace!(
            "HostHandler received request: method={}, params={}",
            method, params
        );
        Err(RpcErrorCode::MethodNotFound)
    }
}
