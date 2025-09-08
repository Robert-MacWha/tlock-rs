// use std::{sync::Arc, thread::sleep, time::Duration};

// use log::{info, trace};
// use tlock_hdk::{
//     plugin::Plugin,
//     tlock_pdk::{
//         api::{Host, HostApi, TlockNamespace},
//         async_trait::async_trait,
//         rpc_message::RpcErrorCode,
//     },
//     typed_plugin::TypedPlugin,
// };

// //? current_thread uses single-threaded mode, simulating the browser environment
// #[tokio::main(flavor = "current_thread")]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     colog::init();

//     info!("Running single-threaded");
//     let wasm_path = "../target/wasm32-wasip1/debug/rust-plugin-template.wasm";
//     let wasm_bytes = std::fs::read(wasm_path)?;
//     info!("Read {} kb from {}", wasm_bytes.len() / 1024, wasm_path);

//     let handler = HostHandler {};
//     let handler = Host(handler);
//     let handler = Arc::new(handler);
//     let plugin = Plugin::new("Test Plugin", wasm_bytes, handler);
//     let plugin = TypedPlugin::new(plugin);

//     let resp = plugin.ping("Hello Plugin!".into()).await?;
//     info!("Received message: {:?}", resp);

//     sleep(Duration::from_millis(1000));

//     Ok(())
// }

// struct HostHandler {}

// impl HostApi<RpcErrorCode> for HostHandler {}

// #[async_trait]
// impl TlockNamespace<RpcErrorCode> for HostHandler {
//     async fn ping(&self, message: String) -> Result<String, RpcErrorCode> {
//         trace!("Host received ping with message: {}", message);
//         Ok(format!("Host Pong: {}", message))
//     }
// }

use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Wrapper that allows trailing elements in arrays
#[derive(Debug)]
pub struct FlexArray<T>(pub T);

impl<'de, T> Deserialize<'de> for FlexArray<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde_json::Value;

        // First get the raw value
        let value = Value::deserialize(deserializer)?;

        // If it's an array, truncate it before deserializing
        let value = match value {
            Value::Array(mut arr) => {
                // Try deserializing with progressively fewer elements until it works
                for len in (0..=arr.len()).rev() {
                    arr.truncate(len);
                    if let Ok(result) = serde_json::from_value(Value::Array(arr.clone())) {
                        return Ok(FlexArray(result));
                    }
                }
                return Err(serde::de::Error::custom("failed to deserialize array"));
            }
            other => other,
        };

        // For non-arrays, deserialize normally
        serde_json::from_value(value)
            .map(FlexArray)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HelloParams {
    pub hello: String, // hello
    pub value: i32,    // value
    #[serde(default)]
    pub value2: Option<i32>, // value2
}

fn main() {
    let instance = HelloParams {
        hello: "world".into(),
        value: 42,
        value2: None,
    };
    let json = serde_json::to_string(&instance).unwrap();
    println!("Serialized: {}", json);

    let deserialized: HelloParams = serde_json::from_str(&json).unwrap();
    println!("Deserialized: {:?}", deserialized);

    let flat_json = r#"["world",42,12]"#;
    let deserialized: HelloParams = serde_json::from_str(&flat_json).unwrap();
    println!("Deserialized from flat_json: {:?}", deserialized);

    let missing_json = r#"["world",42]"#;
    let deserialized: HelloParams = serde_json::from_str(&missing_json).unwrap();
    println!("Deserialized from flat_json: {:?}", deserialized);

    let extra_json = r#"["world",42,12,13,14,15,16]"#;
    let deserialized: FlexArray<HelloParams> = serde_json::from_str(&extra_json).unwrap();
    println!("Deserialized from extra_json: {:?}", deserialized);
}
