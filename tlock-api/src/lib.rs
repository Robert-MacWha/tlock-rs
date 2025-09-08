use std::str::FromStr;

use async_trait::async_trait;
use serde_json::Value;
use wasmi_pdk::{
    api::{ApiError, RequestHandler},
    rpc_message::RpcErrorCode,
};

use crate::{
    methods::Methods, namespace_global::GlobalNamespace, namespace_plugin::PluginNamespace,
};

pub mod flex_array;
pub mod methods;
pub mod namespace_eth;
pub mod namespace_global;
pub mod namespace_plugin;

pub trait PluginApi<E: ApiError>: GlobalNamespace<E> + PluginNamespace<E> {}
pub trait HostApi<E: ApiError>: GlobalNamespace<E> {}

pub struct Plugin<T>(pub T);
pub struct Host<T>(pub T);

macro_rules! register_methods {
    ($self:expr, $method:expr, $params:expr, {
        $($variant:ident => $fn_name:ident($($param_type:ty)?)),* $(,)?
    }) => {
        match $method {
            $(
                Methods::$variant => {
                    register_methods!(@call $self, $fn_name, $params, $($param_type)?)
                }
            )*
            _ => Err(RpcErrorCode::MethodNotFound.into())
        }
    };

    // Handle function with one param
    (@call $self:expr, $fn_name:ident, $params:expr, $param_type:ty) => {
        {
            let param: $param_type = serde_json::from_value($params).map_err(|_| RpcErrorCode::InvalidParams)?;
            let result = $self.0.$fn_name(param).await?;
            let result = serde_json::to_value(result).map_err(|_| RpcErrorCode::ParseError)?;
            Ok(result)
        }
    };

    // Handle function with no params
    (@call $self:expr, $fn_name:ident, $params:expr, ) => {
        {
            let result = $self.0.$fn_name().await?;
            let result = serde_json::to_value(result).map_err(|_| RpcErrorCode::ParseError)?;
            Ok(result)
        }
    };
}

#[async_trait]
impl<T, E> RequestHandler<E> for Host<T>
where
    T: HostApi<E> + Send + Sync,
    E: ApiError,
{
    async fn handle(&self, method: &str, params: Value) -> Result<Value, E> {
        let m: Methods = Methods::from_str(method).map_err(|_| RpcErrorCode::MethodNotFound)?;

        register_methods!(self, m, params, {
            TlockPing => ping(String),
        })
    }
}

#[async_trait]
impl<T, E> RequestHandler<E> for Plugin<T>
where
    T: PluginApi<E> + Send + Sync,
    E: ApiError,
{
    async fn handle(&self, method: &str, params: Value) -> Result<Value, E> {
        let m: Methods = Methods::from_str(method).map_err(|_| RpcErrorCode::MethodNotFound)?;

        register_methods!(self, m, params, {
            TlockPing     => ping(String),
            PluginVersion => version(),
            PluginName    => name(),
        })
    }
}
