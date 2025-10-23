use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tlock_api::{RpcMethod, host};
use wasmi_pdk::{api::ApiError, rpc_message::RpcErrorCode, tracing::error, transport::Transport};

pub async fn get_state<T, R, E>(transport: Arc<T>) -> Result<R, E>
where
    T: Transport<E> + Send + Sync + 'static,
    R: DeserializeOwned,
    E: ApiError + From<RpcErrorCode>,
{
    let state_bytes = host::GetState.call(transport, ()).await?.ok_or_else(|| {
        error!("State is empty");
        E::from(RpcErrorCode::InternalError)
    })?;

    let state: R = serde_json::from_slice(&state_bytes).map_err(|e| {
        error!("Failed to deserialize state: {}", e);
        E::from(RpcErrorCode::InternalError)
    })?;

    Ok(state)
}

pub async fn set_state<T, S, E>(transport: Arc<T>, state: &S) -> Result<(), E>
where
    T: Transport<E> + Send + Sync + 'static,
    S: Serialize,
    E: ApiError + From<RpcErrorCode>,
{
    let state_bytes = serde_json::to_vec(state).map_err(|e| {
        error!("Failed to serialize state: {}", e);
        E::from(RpcErrorCode::InternalError)
    })?;

    host::SetState.call(transport, state_bytes).await
}
