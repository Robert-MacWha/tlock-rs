pub use tlock_api;
pub use wasmi_hdk;
pub use wasmi_pdk;
pub mod dispatcher;

#[macro_export]
macro_rules! impl_rpc_handler {
    (
        $provider:ty, $method:ty,
        |$self:ident, $plugin_id:ident, $params_name:ident| $body:expr
    ) => {
        #[cfg(target_arch = "wasm32")]
        impl $crate::dispatcher::RpcHandler<$method> for $provider {
            fn invoke(
                &$self,
                $plugin_id: $crate::wasmi_hdk::plugin::PluginId,
                $params_name: <$method as $crate::tlock_api::RpcMethod>::Params,
            ) -> impl core::future::Future<
                Output = Result<
                    <$method as $crate::tlock_api::RpcMethod>::Output,
                    $crate::wasmi_pdk::rpc_message::RpcErrorCode,
                >,
            > + '_ {
                async move { $body }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        impl $crate::dispatcher::RpcHandler<$method> for $provider {
            fn invoke(
                &$self,
                $plugin_id: $crate::wasmi_hdk::plugin::PluginId,
                $params_name: <$method as $crate::tlock_api::RpcMethod>::Params,
            ) -> impl core::future::Future<
                Output = Result<
                    <$method as $crate::tlock_api::RpcMethod>::Output,
                    $crate::wasmi_pdk::rpc_message::RpcErrorCode,
                >,
            > + Send + '_ {
                async move { $body }
            }
        }
    };
}
