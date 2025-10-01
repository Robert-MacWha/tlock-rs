use ::host::host::Host;
use log::info;
use std::{sync::Arc, thread::sleep, time::Duration};
use tlock_hdk::{
    dispatcher::Dispatcher,
    tlock_api::{entities::VaultId, global, host, vault},
    wasmi_hdk::plugin::{Plugin, PluginId},
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

    let host = Host::new();
    let host = Arc::new(host);
    let mut dispatcher = Dispatcher::new(host.clone());
    dispatcher.register::<global::Ping>();
    dispatcher.register::<host::RegisterEntity>();
    dispatcher.register::<host::GetState>();
    dispatcher.register::<host::SetState>();
    dispatcher.register::<vault::BalanceOf>();
    dispatcher.register::<vault::Transfer>();
    dispatcher.register::<vault::GetReceiptAddress>();
    dispatcher.register::<vault::OnReceive>();

    let dispatcher = Arc::new(dispatcher);

    // let template_plugin_id = load_plugin(
    //     "target/wasm32-wasip1/release/plugin-template.wasm",
    //     "Template Plugin",
    //     "0001".into(),
    //     host.clone(),
    //     dispatcher.clone(),
    // )
    // .await?;
    let example_vault_id = load_plugin(
        "target/wasm32-wasip1/release/example-vault.wasm",
        "Example Vault Plugin",
        "0002".into(),
        host.clone(),
        dispatcher.clone(),
    )
    .await?;

    // host.ping_plugin(&template_plugin_id).await?;
    host.ping_plugin(&example_vault_id).await?;

    host.list_entities()
        .iter()
        .for_each(|e| info!("Looked up registered entity: {}", e));

    match host.balance_of(VaultId::new("bla".into())).await {
        Ok(bal) => info!("Unexpected balance for unknown vault: {:?}", bal),
        Err(e) => info!("Expected error for unknown vault: {}", e),
    }

    match host
        .balance_of(VaultId::new(
            "1:0x0102030405060708090a0B0c0d0e0f1011121314".into(),
        ))
        .await
    {
        Ok(bal) => info!("Balance for known vault: {:?}", bal),
        Err(e) => info!("Error fetching balance for known vault: {}", e),
    }

    sleep(Duration::from_millis(1000));
    Ok(())
}

async fn load_plugin(
    wasm_path: &str,
    name: &str,
    id: PluginId,
    host: Arc<Host>,
    dispatcher: Arc<Dispatcher<Host>>,
) -> Result<PluginId, Box<dyn std::error::Error>> {
    let wasm_bytes = std::fs::read(wasm_path)?;
    info!("Read {} kb from {}", wasm_bytes.len() / 1024, wasm_path);

    let plugin = Plugin::new(name, id, wasm_bytes, dispatcher)?;
    let plugin = Arc::new(plugin);

    host.register_plugin(plugin.clone()).await?;

    Ok(plugin.id())
}
