use std::sync::Arc;

use serde_json::Value;
use wasmi_pdk::{
    rpc_message::RpcError,
    server::{BoxFuture, Server},
};

use crate::{host_handler::HostHandler, plugin::PluginId};

impl<S: Clone + Send + Sync + 'static> HostHandler for Server<(Option<PluginId>, S)> {
    fn handle<'a>(
        &'a self,
        plugin: PluginId,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, RpcError>> {
        Box::pin(async move {
            let s = self.state().1.clone();
            let s = Arc::new((Some(plugin), s));
            self.handle_with_state(s, method, params).await
        })
    }
}
