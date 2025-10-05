// TODO: Merge this + the integration test version to avoid duplication & inconsistency
use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc};
use tokio::runtime::Builder;
use tracing::info;
use wasmi_hdk::host_handler::HostHandler;
use wasmi_hdk::plugin::{Plugin, PluginId};
use wasmi_pdk::transport::Transport;
use wasmi_pdk::{async_trait::async_trait, rpc_message::RpcErrorCode};

fn load_plugin_wasm() -> Vec<u8> {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/release/test-plugin.wasm");

    info!("Loading plugin WASM from {:?}", wasm_path);
    fs::read(wasm_path).expect("Failed to read plugin WASM file")
}

struct MyHostHandler {}
#[async_trait]
impl HostHandler for MyHostHandler {
    async fn handle(
        &self,
        _id: PluginId,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcErrorCode> {
        match method {
            "echo" => Ok(params),
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

/// Benchmark the prime sieve function with a small input. Primarily tests the overhead
/// of calling into the wasm module.
pub fn bench_prime_sieve_small(c: &mut Criterion) {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes.clone(), handler).unwrap();

    c.bench_function("prime_sieve_small", |b| {
        b.iter(|| {
            let fut = async {
                plugin
                    .call("prime_sieve", Value::Number(1.into()))
                    .await
                    .unwrap();
            };

            rt.block_on(fut);
        })
    });
}

/// Benchmark the prime sieve function with a large input. Tests both the overhead
/// of calling into the wasm module and performance within the wasm module.
pub fn bench_prime_sieve_large(c: &mut Criterion) {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes.clone(), handler).unwrap();

    c.bench_function("prime_sieve_large", |b| {
        b.iter(|| {
            let fut = async {
                plugin
                    .call("prime_sieve", Value::Number(100_000.into()))
                    .await
                    .unwrap();
            };

            rt.block_on(fut);
        })
    });
}

/// Benchmark sending many echo requests to the host, and receiving responses.
pub fn bench_echo_many(c: &mut Criterion) {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes.clone(), handler).unwrap();

    c.bench_function("many_echo", |b| {
        b.iter(|| {
            let fut = async {
                plugin
                    .call("many_echo", Value::Number(200.into()))
                    .await
                    .unwrap();
            };

            rt.block_on(fut);
        })
    });
}

criterion_group!(
    benches,
    bench_prime_sieve_small,
    bench_prime_sieve_large,
    bench_echo_many
);
criterion_main!(benches);
