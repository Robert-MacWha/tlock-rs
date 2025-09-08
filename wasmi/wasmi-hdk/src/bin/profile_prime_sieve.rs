use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc};
use wasmi_hdk::plugin::Plugin;
use wasmi_pdk::{api::RequestHandler, async_trait::async_trait, rpc_message::RpcErrorCode};

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

fn load_plugin_wasm() -> Vec<u8> {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/release/test-plugin.wasm");
    fs::read(wasm_path).expect("Failed to read plugin WASM file")
}

#[tokio::main]
async fn main() {
    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});
    let plugin = Plugin::new("test_plugin", wasm_bytes, handler).unwrap();

    for i in 0..1000 {
        println!("Iteration {}/1000", i + 1);

        plugin
            .call("prime_sieve", Value::Number(100_000.into()))
            .await
            .unwrap();
    }
}
