use std::{fs, path::PathBuf, sync::Arc};

use criterion::{Criterion, criterion_group, criterion_main};
use log::info;
use serde_json::Value;
use tokio::runtime::Builder;
use wasmi_hdk::plugin::Plugin;
use wasmi_pdk::{api::RequestHandler, async_trait::async_trait, rpc_message::RpcErrorCode};

fn load_plugin_wasm() -> Vec<u8> {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/release/test-plugin.wasm");

    info!("Loading plugin WASM from {:?}", wasm_path);
    fs::read(wasm_path).expect("Failed to read plugin WASM file")
}

struct MyHostHandler {}

#[async_trait]
impl RequestHandler<RpcErrorCode> for MyHostHandler {
    async fn handle(&self, _method: &str, _params: Value) -> Result<Value, RpcErrorCode> {
        Err(RpcErrorCode::MethodNotFound)
    }
}

/// Benchmark the prime sieve function with a small input. Primarily tests the overhead
/// of calling into the wasm module.
pub fn bench_prime_sieve_small(c: &mut Criterion) {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    info!("Starting small prime sieve test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let plugin = Plugin::new("test_plugin", wasm_bytes.clone(), handler);

    c.bench_function("prime_sieve", |b| {
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

    info!("Starting large prime sieve test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let plugin = Plugin::new("test_plugin", wasm_bytes.clone(), handler);

    c.bench_function("prime_sieve", |b| {
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

criterion_group!(benches, bench_prime_sieve_small, bench_prime_sieve_large);
criterion_main!(benches);
