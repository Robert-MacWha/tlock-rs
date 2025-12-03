use serde_json::Value;
use std::sync::Arc;
use tracing::info;
use wasmi_hdk::{plugin::Plugin, server::HostServer};
use wasmi_pdk::transport::Transport;
use web_time::{Instant, SystemTime};

#[cfg(target_family = "wasm")]
use wasm_bindgen_test::*;

#[cfg(not(target_family = "wasm"))]
use tracing_test::traced_test;

const PLUGIN_WASM: &[u8] = include_bytes!("../../../target/wasm32-wasip1/release/test-plugin.wasm");

fn load_plugin_wasm() -> Vec<u8> {
    PLUGIN_WASM.to_vec()
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

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(not(target_family = "wasm"), traced_test)]
async fn test_plugin() {
    info!("Starting test_plugin...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();
    plugin.call("ping", Value::Null).await.unwrap();
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(not(target_family = "wasm"), traced_test)]
async fn test_get_random_number() {
    info!("Starting get_random_number test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();
    let response = plugin.call("get_random_number", Value::Null).await.unwrap();

    info!("Random number response: {:?}", response);
    let number = response.result.as_u64().unwrap();
    assert!(number <= u64::MAX);
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(not(target_family = "wasm"), traced_test)]
async fn test_get_time() {
    info!("Starting get_time test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();
    let response = plugin.call("get_time", Value::Null).await.unwrap();

    info!("Get time response: {:?}", response);

    let timestamp = response.result.as_u64().unwrap();
    let now = SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let diff = timestamp.abs_diff(now);
    assert!(diff < 2, "Timestamp difference too large: {}", diff);
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(not(target_family = "wasm"), traced_test)]
async fn test_sleep() {
    info!("Starting sleep test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();

    let sleep_duration = 1500; // milliseconds
    let start = Instant::now();
    plugin
        .call("sleep", Value::Number(sleep_duration.into()))
        .await
        .unwrap();
    let elapsed = start.elapsed().as_millis();

    assert!(
        elapsed >= sleep_duration as u128,
        "Sleep duration too short: {} ms",
        elapsed
    );
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(not(target_family = "wasm"), traced_test)]
async fn test_many_echo() {
    info!("Starting many echo test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();
    plugin
        .call("many_echo", Value::Number(200.into()))
        .await
        .unwrap();
}

#[cfg_attr(target_family = "wasm", wasm_bindgen_test)]
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(not(target_family = "wasm"), traced_test)]
async fn test_prime_sieve() {
    info!("Starting prime sieve test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(get_host_server());

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();

    let response = plugin
        .call("prime_sieve", Value::Number(1000.into()))
        .await
        .unwrap();

    info!("Prime sieve response: {:?}", response);
    let count = response.result["count"].as_u64().unwrap();
    assert_eq!(count, 168);
}
