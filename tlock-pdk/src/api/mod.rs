use std::str::FromStr;

use serde_json::Value;

use crate::{api::methods::Methods, rpc_message::RpcErrorCode};

pub mod methods;

pub trait PluginApi<E>: TlockNamespace<E> + PluginNamespace<E>
where
    E: From<RpcErrorCode>,
{
}
pub trait HostApi<E: From<RpcErrorCode>>: TlockNamespace<E>
where
    E: From<RpcErrorCode>,
{
}

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

    // Handle function with one parameter
    (@call $self:expr, $fn_name:ident, $params:expr, $param_type:ty) => {
        {
            let param: $param_type = serde_json::from_value($params).map_err(|_| RpcErrorCode::InvalidParams)?;
            let result = $self.0.$fn_name(param)?;
            let result = serde_json::to_value(result).map_err(|_| RpcErrorCode::ParseError)?;
            Ok(result)
        }
    };

    // Handle function with no parameters
    (@call $self:expr, $fn_name:ident, $params:expr, ) => {
        {
            let result = $self.0.$fn_name()?;
            let result = serde_json::to_value(result).map_err(|_| RpcErrorCode::ParseError)?;
            Ok(result)
        }
    };
}

impl<T, E> RequestHandler<E> for Plugin<T>
where
    T: PluginApi<E>,
    E: From<RpcErrorCode>,
{
    fn handle(&self, method: &str, params: Value) -> Result<Value, E> {
        let m: Methods = Methods::from_str(method).map_err(|_| RpcErrorCode::MethodNotFound)?;

        register_methods!(self, m, params, {
            TlockPing     => ping(String),
            PluginVersion => version(),
            PluginName    => name(),
        })
    }
}

impl<T, E> RequestHandler<E> for Host<T>
where
    T: HostApi<E>,
    E: From<RpcErrorCode>,
{
    fn handle(&self, method: &str, params: Value) -> Result<Value, E> {
        let m: Methods = Methods::from_str(method).map_err(|_| RpcErrorCode::MethodNotFound)?;

        register_methods!(self, m, params, {
            TlockPing => ping(String),
        })
    }
}

pub trait RequestHandler<E: From<RpcErrorCode>> {
    fn handle(&self, method: &str, params: Value) -> Result<Value, E>;
}

pub trait TlockNamespace<E: From<RpcErrorCode>> {
    fn ping(&self, _msg: String) -> Result<String, E> {
        Ok("Pong".into())
    }
}

pub trait PluginNamespace<E: From<RpcErrorCode>> {
    fn name(&self) -> Result<String, E>;
    fn version(&self) -> Result<String, E>;
}
