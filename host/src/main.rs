use log::{info, trace};
use std::{sync::Arc, thread::sleep, time::Duration};
use tlock_hdk::{
    async_trait::async_trait,
    tlock_api::{
        CompositeClient, CompositeServer,
        domains::tlock::{TlockDomain, TlockDomainServer},
    },
    wasmi_hdk::{plugin::Plugin, wasmi_pdk::rpc_message::RpcErrorCode},
};

struct HostHandler {}

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

    let handler = HostHandler {};
    let handler = Arc::new(handler);
    let mut server = CompositeServer::new();
    server.register(TlockDomainServer::new(handler.clone()));
    let server = Arc::new(server);

    let plugin = Plugin::new("Test Plugin", wasm_bytes, server.clone())?;
    let plugin = CompositeClient::new(Arc::new(plugin));

    let resp = plugin.tlock().ping("Hello Plugin!".into()).await?;
    info!("Received message: {:?}", resp);

    sleep(Duration::from_millis(1000));

    Ok(())
}

#[async_trait]
impl TlockDomain for HostHandler {
    type Error = RpcErrorCode;

    async fn ping(&self, message: String) -> Result<String, Self::Error> {
        trace!("Host received ping with message: {}", message);
        Ok(format!("Host Pong: {}", message))
    }

    async fn name(&self) -> Result<String, Self::Error> {
        Ok("Host".to_string())
    }

    async fn version(&self) -> Result<String, Self::Error> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }
}
