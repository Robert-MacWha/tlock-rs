pub use tlock_api;
pub use tracing;
pub use wasmi_plugin_hdk;
pub use wasmi_plugin_pdk;
pub mod server;

#[macro_export]
macro_rules! __impl_host_rpc_base {
    ($host_ty:ty, $method:ty, $host_fn:ident, $call_expr:expr) => {
        pub async fn $host_fn(
            host: (
                $crate::wasmi_plugin_hdk::plugin_id::PluginId,
                ::std::sync::Weak<$host_ty>,
            ),
            params: <$method as $crate::tlock_api::RpcMethod>::Params,
        ) -> Result<
            <$method as $crate::tlock_api::RpcMethod>::Output,
            $crate::wasmi_plugin_pdk::rpc_message::RpcError,
        > {
            use $crate::{
                tracing::{info, warn},
                wasmi_plugin_pdk::rpc_message::RpcErrorContext,
            };

            let plugin_id = &host.0;
            let host = host.1.upgrade().context("Host has been dropped")?;

            $call_expr(host, plugin_id.clone(), params).await
        }
    };
}

#[macro_export]
macro_rules! impl_host_rpc {
    ($host_ty:ty, $method:ty, $host_fn:ident) => {
        $crate::__impl_host_rpc_base!(
            $host_ty,
            $method,
            $host_fn,
            |host: ::std::sync::Arc<$host_ty>,
             plugin_id: $crate::wasmi_plugin_hdk::plugin_id::PluginId,
             params: <$method as $crate::tlock_api::RpcMethod>::Params| async move {
                host.$host_fn(&plugin_id, params).await
            }
        );
    };
}

#[macro_export]
macro_rules! impl_host_rpc_no_id {
    ($host_ty:ty, $method:ty, $host_fn:ident) => {
        $crate::__impl_host_rpc_base!(
            $host_ty,
            $method,
            $host_fn,
            |host: ::std::sync::Arc<$host_ty>,
             _plugin_id: $crate::wasmi_plugin_hdk::plugin_id::PluginId,
             params: <$method as $crate::tlock_api::RpcMethod>::Params| async move {
                host.log_event(format!("[{}] {}", _plugin_id, <$method>::NAME));
                host.$host_fn(params).await
            }
        );
    };
}
