use std::str::FromStr;

use alloy_rpc_types::TransactionRequest;
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    api::{eth::EthNamespace, methods::Methods},
    rpc_message::RpcErrorCode,
};

pub mod eth;
pub mod methods;

pub trait ApiError: From<RpcErrorCode> + Send + Sync {}
impl<T> ApiError for T where T: From<RpcErrorCode> + Send + Sync {}

#[async_trait]
pub trait RequestHandler<E: ApiError>: Send + Sync {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, E>;
}

#[async_trait]
pub trait TlockNamespace<E: ApiError>: Send + Sync {
    async fn ping(&self, _msg: String) -> Result<String, E> {
        Ok("Pong".into())
    }
}

#[async_trait]
pub trait PluginNamespace<E: ApiError>: Send + Sync {
    async fn name(&self) -> Result<String, E>;
    async fn version(&self) -> Result<String, E>;
}

pub trait PluginApi<E: ApiError>: TlockNamespace<E> + PluginNamespace<E> {}
pub trait HostApi<E: ApiError>: TlockNamespace<E> {}

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
            _ => Err(RpcErrorCode::MethodNotSupported.into())
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
