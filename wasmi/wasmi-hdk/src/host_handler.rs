use serde_json::Value;
use wasmi_pdk::{rpc_message::RpcErrorCode, server::BoxFuture};

use crate::plugin::PluginId;

/// HostHandler is a wrapper around the `wasmi-pdk::RequestHandler` trait that adds
/// the plugin ID as the first arg in the `handle` method. It should be used
/// by hosts that manage multiple plugins to differentiate request sources.
pub trait HostHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        plugin: PluginId,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, RpcErrorCode>>;
}
