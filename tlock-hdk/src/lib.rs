pub use tlock_api;
pub use tlock_pdk;
pub use tracing;
pub use wasmi_hdk;
pub use wasmi_pdk;

#[macro_export]
macro_rules! __impl_host_rpc_base {
    ($host_ty:ty, $method:ty, $host_fn:ident, $call_expr:expr) => {
        pub async fn $host_fn(
            host: ::std::sync::Arc<(
                Option<$crate::wasmi_hdk::plugin::PluginId>,
                ::std::sync::Weak<$host_ty>,
            )>,
            params: <$method as $crate::tlock_api::RpcMethod>::Params,
        ) -> Result<
            <$method as $crate::tlock_api::RpcMethod>::Output,
            $crate::wasmi_pdk::rpc_message::RpcError,
        > {
            use $crate::tracing::{info, warn};

            let plugin_id = host.0.as_ref().unwrap();
            let host = host.1.upgrade().ok_or_else(|| {
                warn!("Host has been dropped");
                $crate::wasmi_pdk::rpc_message::RpcError::InternalError
            })?;

            info!("[host_func] Plugin {} sent {}", plugin_id, <$method>::NAME);
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
             plugin_id: $crate::wasmi_hdk::plugin::PluginId,
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
             _plugin_id: $crate::wasmi_hdk::plugin::PluginId,
             params: <$method as $crate::tlock_api::RpcMethod>::Params| async move {
                host.$host_fn(params).await
            }
        );
    };
}
