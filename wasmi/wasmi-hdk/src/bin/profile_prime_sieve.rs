use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc};
use wasmi_hdk::{
    host_handler::HostHandler,
    plugin::{Plugin, PluginId},
};
use wasmi_pdk::{async_trait::async_trait, rpc_message::RpcError, transport::Transport};

struct MyHostHandler {}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl HostHandler for MyHostHandler {
    async fn handle(
        &self,
        _plugin: PluginId,
        method: &str,
        _params: Value,
    ) -> Result<Value, RpcError> {
        match method {
            "echo" => Ok(Value::String("echo".to_string())),
            _ => Err(RpcError::MethodNotFound),
        }
    }
}

fn load_plugin_wasm() -> Vec<u8> {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/release/test-plugin.wasm");
    fs::read(wasm_path).expect("Failed to read plugin WASM file")
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let wasm_bytes = load_plugin_wasm();
    let handler = Arc::new(MyHostHandler {});
    let id = "0001".into();
    let plugin = Plugin::new("test_plugin", &id, wasm_bytes, handler).unwrap();

    for i in 0..1000 {
        println!("Iteration {}/1000", i + 1);

        plugin
            .call("prime_sieve", Value::Number(100_000.into()))
            .await
            .unwrap();
    }
}
