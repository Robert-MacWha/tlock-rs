use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use log::info;
use serde_json::Value;
use tokio::time::{sleep, timeout};
use wasmi_hdk::plugin::Plugin;
use wasmi_pdk::{api::RequestHandler, async_trait::async_trait, rpc_message::RpcErrorCode};

fn load_plugin_wasm() -> Vec<u8> {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/debug/test-plugin.wasm");

    info!("Loading plugin WASM from {:?}", wasm_path);
    fs::read(wasm_path).expect("Failed to read plugin WASM file")
}

struct MyHostHandler {}

#[async_trait]
impl RequestHandler<RpcErrorCode> for MyHostHandler {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcErrorCode> {
        info!("Host received method: {}, params: {:?}", method, params);

        sleep(Duration::from_millis(100)).await; // Simulate some processing delay

        info!("Host processing complete for method: {}", method);

        match method {
            "ping" => Ok(serde_json::json!("pong")),
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

#[tokio::test]
async fn test_plugin() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .with_colors(true)
        .init()
        .ok();

    log::info!("Starting test_plugin...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let result = timeout(Duration::from_secs(1), async {
        info!("Running test...");
        let plugin = Plugin::new("test_plugin", wasm_bytes, handler).unwrap();
        plugin.call("ping", Value::Null).await.unwrap();
    })
    .await;

    result.expect("Test timed out");
}

#[tokio::test]
async fn test_prime_sieve() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .with_colors(true)
        .init()
        .ok();

    info!("Starting prime sieve test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let result = timeout(Duration::from_secs(2), async {
        let plugin = Plugin::new("test_plugin", wasm_bytes, handler).unwrap();

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
