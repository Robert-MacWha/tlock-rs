use tlock_pdk::{plugin_host::PluginHost, transport::Transport};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_path = "plugin.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;

    let mut plugin = PluginHost::spawn(wasm_bytes, None)?;

    let response = plugin.call("method_1", serde_json::json!({"param1": "value1"}))?;
    let msg = response.recv()?;
    println!("Received message: {:?}", msg);

    Ok(())
}
