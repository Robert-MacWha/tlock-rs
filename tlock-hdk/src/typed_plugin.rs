use tlock_pdk::{
    api::{PluginApi, PluginNamespace, TlockNamespace, methods::Methods},
    rpc_message::RpcErrorCode,
};

use crate::plugin::{Plugin, PluginError};

/// TypedPlugin is a type-safe wrapped plugin
pub struct TypedPlugin<'a> {
    plugin: Plugin<'a>,
}

impl<'a> TypedPlugin<'a> {
    pub fn new(plugin: Plugin<'a>) -> Self {
        Self { plugin }
    }
}

macro_rules! generate_rpc_methods {
    (
        $(
            $fn_name:ident ( $( $arg_name:ident : $arg_ty:ty ),* $(,)? ) -> $method:path
        ),* $(,)?
    ) => {
        $(
            fn $fn_name(&self, $( $arg_name : $arg_ty ),* ) -> Result<String, PluginError> {
                let value = serde_json::to_value(($( $arg_name ),*)).map_err(|_| RpcErrorCode::ParseError)?;
                let res = self.plugin.call(&$method.to_string(), value)?;

                let res: String = serde_json::from_value(res.result).map_err(|_| RpcErrorCode::ParseError)?;
                Ok(res)
            }
        )*
    };
}

impl PluginApi<PluginError> for TypedPlugin<'_> {}

impl PluginNamespace<PluginError> for TypedPlugin<'_> {
    generate_rpc_methods! {
        name() -> Methods::PluginName,
        version() -> Methods::PluginVersion
    }
}

impl TlockNamespace<PluginError> for TypedPlugin<'_> {
    generate_rpc_methods! {
        ping(value: String) -> Methods::TlockPing
    }
}
