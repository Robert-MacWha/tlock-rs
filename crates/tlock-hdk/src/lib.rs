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
                $crate::wasmi_plugin_hdk::instance_id::InstanceId,
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

            let instance_id = &host.0;
            let host = host.1.upgrade().context("Host has been dropped")?;

            $call_expr(host, *instance_id, params).await
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
             instance_id: $crate::wasmi_plugin_hdk::instance_id::InstanceId,
             params: <$method as $crate::tlock_api::RpcMethod>::Params| async move {
                let span = ::tracing::info_span!(
                    <$method>::NAME,
                    plugin = %instance_id.plugin,
                );
                let _enter = span.enter();
                host.$host_fn(&instance_id, params).await
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
             instance_id: $crate::wasmi_plugin_hdk::instance_id::InstanceId,
             params: <$method as $crate::tlock_api::RpcMethod>::Params| async move {
                let plugin_id = &instance_id.plugin;
                let span = ::tracing::info_span!(
                    <$method>::NAME,
                    plugin = %plugin_id,
                );
                let _enter = span.enter();
                let plugin_name = host.get_plugin(plugin_id).map(|p| p.name().to_string()).unwrap_or("<unknown>".to_string());
                host.log_event(<$method>::NAME, Some(&plugin_name));
                host.$host_fn(params).await
            }
        );
    };
}
