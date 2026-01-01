use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};
use tlock_api::{RpcMethod, host};
use wasmi_plugin_pdk::{
    rpc_message::{RpcError, RpcErrorContext},
    transport::SyncTransport,
};

pub fn get_state<T, R, E>(transport: T) -> R
where
    T: SyncTransport<E> + Send + Sync + 'static,
    R: DeserializeOwned + Default,
    E: Into<RpcError>,
{
    try_get_state::<T, R, E>(transport).unwrap_or_default()
}

pub fn try_get_state<T, R, E>(transport: T) -> Result<R, RpcError>
where
    T: SyncTransport<E> + Send + Sync + 'static,
    R: DeserializeOwned,
    E: Into<RpcError>,
{
    let state_bytes = host::GetState.call(transport, ())?.context("Empty state")?;
    let state = serde_json::from_slice(&state_bytes)
        .with_context(|| format!("Failed to deserialize state from bytes: {:?}", state_bytes))?;
    Ok(state)
}

pub fn set_state<T, S, E>(transport: T, state: &S) -> Result<(), RpcError>
where
    T: SyncTransport<E> + Send + Sync + 'static,
    S: Debug + Serialize,
    E: Into<RpcError>,
{
    let state_bytes = serde_json::to_vec(state)
        .with_context(|| format!("Failed to serialize state {:?}", state))?;
    host::SetState.call(transport, state_bytes)
}
