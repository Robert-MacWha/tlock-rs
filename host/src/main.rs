use log::info;
use std::{sync::Arc, thread::sleep, time::Duration};
use tlock_hdk::{
    dispatcher::{Dispatcher, HostRpcHandler},
    tlock_api::{Ping, RpcMethod},
    wasmi_hdk::{
        plugin::{Plugin, PluginId},
        wasmi_pdk::{async_trait::async_trait, rpc_message::RpcErrorCode},
    },
    wasmi_pdk::transport::Transport,
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
    let wasm_path = "target/wasm32-wasip1/release/plugin-template.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;
    info!("Read {} kb from {}", wasm_bytes.len() / 1024, wasm_path);

    let host = Host {};
    let mut dispatcher = Dispatcher::new(host);
    dispatcher.register::<Ping>();
    let dispatcher = Arc::new(dispatcher);

    let plugin = Plugin::new("Test Plugin", "0001".into(), wasm_bytes, dispatcher)?;
    let plugin = Arc::new(plugin);
    let resp = Ping.call(plugin, ()).await?;

    sleep(Duration::from_millis(1000));

    Ok(())
}

#[async_trait]
impl HostRpcHandler<Ping> for Host {
    async fn invoke(&self, _plugin_id: PluginId, _params: ()) -> Result<String, RpcErrorCode> {
        Ok("pong from host".to_string())
    }
}
