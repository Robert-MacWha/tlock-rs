use ::host::host::Host;
use log::info;
use std::{sync::Arc, thread::sleep, time::Duration};
use tlock_hdk::tlock_api::entities::VaultId;

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

    let wasm_bytes = std::fs::read("target/wasm32-wasip1/release/example-vault.wasm")?;
    let example_vault_id = host
        .load_plugin(&wasm_bytes, "Example Vault Plugin")
        .await?;

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
