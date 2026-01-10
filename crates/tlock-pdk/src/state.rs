use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tlock_api::{
    RpcMethod,
    state::{self, SetError},
};
use wasmi_plugin_pdk::{rpc_message::RpcError, transport::SyncTransport};

#[derive(Debug, Error)]
pub enum LockError {
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("State is empty")]
    Empty,
    #[error("Set key error: {0}")]
    SetError(#[from] SetError),
}

pub trait SyncStateGuard: SyncTransport<RpcError> + Clone + Sized {
    fn lock<V: Serialize + DeserializeOwned + Default>(
        &self,
        key: impl Into<String>,
    ) -> Result<LockedState<Self, V>, LockError> {
        let key = key.into();
        let data = state::LockKey.call(self.clone(), key.clone())?;

        let value: V = if data.is_empty() {
            V::default()
        } else {
            serde_json::from_slice(&data)?
        };
        Ok(LockedState {
            transport: self.clone(),
            key,
            value,
            dirty: false,
        })
    }

    fn try_lock<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
    ) -> Result<LockedState<Self, V>, LockError> {
        let key = key.into();
        let data = state::LockKey.call(self.clone(), key.clone())?;
        if data.is_empty() {
            return Err(LockError::Empty);
        }
        let value: V = serde_json::from_slice(&data)?;
        Ok(LockedState {
            transport: self.clone(),
            key,
            value,
            dirty: false,
        })
    }
}

pub struct LockedState<T: SyncTransport<RpcError> + Clone, V: Serialize> {
    transport: T,
    key: String,
    value: V,
    dirty: bool,
}

impl<T: SyncTransport<RpcError> + Clone, V: Serialize + Clone> LockedState<T, V> {
    /// Consume the guard, unlocks, and return the value
    pub fn into_inner(self) -> V {
        self.value.clone()
    }
}

impl<T: SyncTransport<RpcError> + Clone, V: Serialize> Deref for LockedState<T, V> {
    type Target = V;
    fn deref(&self) -> &V {
        &self.value
    }
}

impl<T: SyncTransport<RpcError> + Clone, V: Serialize> DerefMut for LockedState<T, V> {
    fn deref_mut(&mut self) -> &mut V {
        self.dirty = true;
        &mut self.value
    }
}

impl<T: SyncTransport<RpcError> + Clone, V: Serialize> Drop for LockedState<T, V> {
    fn drop(&mut self) {
        if self.dirty {
            if let Ok(data) = serde_json::to_vec(&self.value) {
                let _ = state::SetKey.call(self.transport.clone(), (self.key.clone(), data));
            }
        }

        let _ = state::UnlockKey.call(self.transport.clone(), self.key.clone());
    }
}
