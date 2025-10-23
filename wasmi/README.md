# Wasmi Plugin Framework

A plugin framework built on the [Wasmi](https://github.com/wasmi-labs/wasmi) WebAssembly interpreter. Wasmi Plugin Framework is designed to run wasm plugins across many architectures, including natively, on mobile, and web browsers (running wasm within wasm).

**Features**

-   JSON-rpc based host <> plugin communication
-   Uses STDIO for communication with host, making implementing plugins in other languages straightforward
    -   STDIN / STDOUT for JSON-rpc communication
    -   STDERR for logs
-   Async compatible
-   Single-threaded compatible
-   Interpreter-based (works on IOS, thanks to wasmi)

**Limitations**

-   Plugins are single-use. Each host request made to a plugin is made to a new instance of that plugin, meaning by default there is no data persistence.

## Architecture

## Example

Check the `wasmi/test-plugin` and `wasmi/wasmi-hdk/tests` files for reference host / plugin implementations.

### Basic Host

```rust
struct MyHostHandler {}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RequestHandler<RpcErrorCode> for MyHostHandler {
    async fn handle(&self, method: &str, _params: Value) -> Result<Value, RpcErrorCode> {
        match method {
            "echo" => Ok(Value::String("echo".to_string())),
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}
```

### Basic Plugin

```rust
struct MyPlugin {}

impl PluginFactory for MyPlugin {
    fn new(_: Arc<JsonRpcTransport>) -> Self {
        Self {}
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RequestHandler<RpcErrorCode> for MyPlugin {
    async fn handle(
        &self,
        method: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, RpcErrorCode> {
        match method {
            "hello" => Ok(serde_json::json!({"message": "Hello from MyPlugin!"})),
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

register_plugin!(MyPlugin);
```
