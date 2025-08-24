mod plugin;
use tlock_pdk::{api::TlockApi, plugin::Plugin, typed_plugin::TypedPlugin};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_path = "plugin.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;

    let plugin = Plugin::new(wasm_bytes)?;
    let mut plugin = TypedPlugin::new(plugin);

    let resp = plugin.ping("Hello!");
    println!("Received message: {:?}", resp);

    Ok(())
}
