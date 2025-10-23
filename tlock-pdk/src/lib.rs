pub use async_trait;
pub use futures;
pub use tlock_api;
pub use wasmi_pdk;
pub mod dispatcher;
pub mod state;

#[macro_export]
macro_rules! impl_rpc_handler {
    (
        $provider:ty, $method:ty,
        |$self:ident, $params_name:ident| $body:expr
    ) => {
        #[cfg(target_arch = "wasm32")]
        impl $crate::dispatcher::RpcHandler<$method> for $provider {
            fn invoke(
                &$self,
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
