use std::{fmt::Debug, sync::Arc};

use serde::{Serialize, de::DeserializeOwned};
use tlock_api::{RpcMethod, host};
use tracing::error;
use wasmi_plugin_pdk::{api::ApiError, rpc_message::RpcError, transport::Transport};

pub async fn get_state<T, R, E>(transport: Arc<T>) -> R
where
    T: Transport<E> + Send + Sync + 'static,
    R: DeserializeOwned + Default,
    E: ApiError + From<RpcError>,
{
    try_get_state::<T, R, E>(transport)
        .await
        .unwrap_or_default()
}

pub async fn try_get_state<T, R, E>(transport: Arc<T>) -> Result<R, E>
where
    T: Transport<E> + Send + Sync + 'static,
    R: DeserializeOwned,
    E: ApiError + From<RpcError>,
{
    let state_bytes = host::GetState.call(transport, ()).await?.ok_or_else(|| {
        error!("State is empty");
        E::from(RpcError::InternalError)
    })?;

    let state: R = serde_json::from_slice(&state_bytes).map_err(|e| {
        error!("Failed to deserialize state: {}", e);
        E::from(RpcError::InternalError)
    })?;

    Ok(state)
}

pub async fn set_state<T, S, E>(transport: Arc<T>, state: &S) -> Result<(), E>
where
    T: Transport<E> + Send + Sync + 'static,
    S: Debug + Serialize,
    E: ApiError + From<RpcError>,
{
    let state_bytes = serde_json::to_vec(state).map_err(|e| {
        error!("Failed to serialize state {:?}: {}", state, e);
        E::from(RpcError::InternalError)
    })?;

    host::SetState.call(transport, state_bytes).await
}
