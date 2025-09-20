use std::{fs, path::PathBuf, sync::Arc};

use criterion::{Criterion, criterion_group, criterion_main};
use log::info;
use serde_json::Value;
use tokio::runtime::Builder;
use wasmi_hdk::plugin::Plugin;
use wasmi_pdk::{api::RequestHandler, async_trait::async_trait, rpc_message::RpcErrorCode};

fn load_plugin_wasm() -> Vec<u8> {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/release/test-plugin-opt.wasm");

    info!("Loading plugin WASM from {:?}", wasm_path);
    fs::read(wasm_path).expect("Failed to read plugin WASM file")
}

struct MyHostHandler {}

#[async_trait]
impl RequestHandler<RpcErrorCode> for MyHostHandler {
    async fn handle(&self, method: &str, _params: Value) -> Result<Value, RpcErrorCode> {
        match method {
            "echo" => Ok(Value::String("echo".to_string())),
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

/// Benchmark the prime sieve function with a small input. Primarily tests the overhead
/// of calling into the wasm module.
pub fn bench_prime_sieve_small(c: &mut Criterion) {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    info!("Starting small prime sieve test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let plugin = Plugin::new("test_plugin", wasm_bytes.clone(), handler).unwrap();

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

    info!("Starting large prime sieve test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let plugin = Plugin::new("test_plugin", wasm_bytes.clone(), handler).unwrap();

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

pub fn bench_prime_sieve_native(c: &mut Criterion) {
    c.bench_function("prime_sieve_native", |b| {
        b.iter(|| {
            sieve_of_eratosthenes(100_000);
        })
    });
}

/// Benchmark sending many echo requests to the host, and receiving responses.
pub fn bench_echo_many(c: &mut Criterion) {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    info!("Starting many echo test...");

    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});

    let plugin = Plugin::new("test_plugin", wasm_bytes.clone(), handler).unwrap();

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
    bench_prime_sieve_native,
    bench_echo_many
);
criterion_main!(benches);

fn sieve_of_eratosthenes(limit: usize) -> Vec<usize> {
    if limit < 2 {
        return vec![];
    }

    let mut is_prime = vec![true; limit + 1];
    is_prime[0] = false;
    is_prime[1] = false;

    for i in 2..=((limit as f64).sqrt() as usize) {
        if is_prime[i] {
            for j in ((i * i)..=limit).step_by(i) {
                is_prime[j] = false;
            }
        }
    }

    (2..=limit).filter(|&i| is_prime[i]).collect()
}
