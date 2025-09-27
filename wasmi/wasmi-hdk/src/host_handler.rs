use serde_json::Value;
use wasmi_pdk::{async_trait::async_trait, rpc_message::RpcErrorCode};

use crate::plugin::PluginId;

/// HostHandler is a wrapper around the `wasmi-pdk::RequestHandler` trait that adds
/// the plugin ID as the first arg in the `handle` method. It should be used
/// by hosts that manage multiple plugins to differentiate request sources.
#[async_trait]
pub trait HostHandler: Send + Sync {
    /// Handle an incoming request from a plugin.
    async fn handle(
        &self,
        plugin: PluginId,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcErrorCode>;
}
