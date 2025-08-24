use tlock_hdk::{
    plugin::Plugin,
    tlock_pdk::{api::TlockApi, plugin_handler::PluginHandler},
    typed_plugin::TypedPlugin,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_path = "../target/wasm32-wasip1/debug/rust-pdk-template.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;

    let handler = HostHandler {};
    let plugin = Plugin::new(wasm_bytes, &handler);
    let plugin = TypedPlugin::new(plugin);

    let resp = plugin.ping("Hello Plugin!");
    println!("Received message: {:?}", resp);

    Ok(())
}

struct HostHandler {}

impl PluginHandler for HostHandler {}

impl TlockApi for HostHandler {
    fn ping(&self, message: &str) -> String {
        format!("Host Pong: {}", message)
    }

    fn version(&self) -> String {
        "Host 1.0.0".to_string()
    }
}
