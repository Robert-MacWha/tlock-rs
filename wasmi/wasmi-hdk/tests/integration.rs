use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use tokio::time::timeout;
use tracing::info;
use tracing_test::traced_test;
use wasmi_hdk::{plugin::Plugin, server::HostServer};
use wasmi_pdk::transport::Transport;

fn load_plugin_wasm() -> Vec<u8> {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/release/test-plugin.wasm");

    info!("Loading plugin WASM from {:?}", wasm_path);
    fs::read(wasm_path).expect("Failed to read plugin WASM file")
}

fn get_host_server() -> HostServer<()> {
    HostServer::default()
        .with_method("ping", |_, _params: ()| async move {
            info!("Received ping request, sending pong response");
            Ok("pong".to_string())
        })
        .with_method("echo", |_, params: Value| async move {
            info!("Received echo request, returning response");
            Ok(params)
        })
}

#[tokio::test]
#[traced_test]
async fn test_plugin() {
    info!("Starting test_plugin...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let result = timeout(Duration::from_secs(1), async {
        let id = "0001".into();
        let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();
        plugin.call("ping", Value::Null).await.unwrap();
    })
    .await;

    result.expect("Test timed out");
}

#[tokio::test]
#[traced_test]
async fn test_prime_sieve() {
    info!("Starting prime sieve test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let result = timeout(Duration::from_secs(2), async {
        let id = "0001".into();
        let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();

        let response = plugin
            .call("prime_sieve", Value::Number(1000.into()))
            .await
            .unwrap();

        info!("Prime sieve response: {:?}", response);

        let count = response.result["count"].as_u64().unwrap();

        assert_eq!(count, 168);
    })
    .await;

    result.expect("Prime sieve test timed out");
}

#[tokio::test]
#[traced_test]
async fn test_many_echo() {
    info!("Starting many echo test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let result = timeout(Duration::from_secs(5), async {
        let id = "0001".into();
        let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();
        plugin
            .call("many_echo", Value::Number(200.into()))
            .await
            .unwrap();
    })
    .await;

    result.expect("Many echo test timed out");
}
