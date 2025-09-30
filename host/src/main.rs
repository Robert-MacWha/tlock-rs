use ::host::host::Host;
use log::info;
use std::{sync::Arc, thread::sleep, time::Duration};
use tlock_hdk::{
    dispatcher::Dispatcher,
    tlock_api::{RpcMethod, global, host, vault},
    wasmi_hdk::plugin::Plugin,
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
    let wasm_path = "target/wasm32-wasip1/release/plugin-template.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;
    info!("Read {} kb from {}", wasm_bytes.len() / 1024, wasm_path);

    let host = Host::new();
    let host = Arc::new(host);
    let mut dispatcher = Dispatcher::new(host.clone());
    dispatcher.register::<global::Ping>();
    dispatcher.register::<host::CreateEntity>();
    dispatcher.register::<vault::BalanceOf>();
    dispatcher.register::<vault::Transfer>();
    dispatcher.register::<vault::GetReceiptAddress>();
    dispatcher.register::<vault::OnReceive>();

    let dispatcher = Arc::new(dispatcher);

    let plugin = Plugin::new("Test Plugin", "0001".into(), wasm_bytes, dispatcher)?;
    let plugin = Arc::new(plugin);
    host.register_plugin(plugin.clone());

    let resp = global::Ping.call(plugin, ()).await?;

    info!("Response from plugin: {}", resp);

    sleep(Duration::from_millis(1000));

    Ok(())
}
