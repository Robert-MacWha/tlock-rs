use std::{
    fmt::Debug,
    marker::PhantomData,
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

impl From<LockError> for RpcError {
    fn from(err: LockError) -> Self {
        RpcError::custom(err.to_string())
    }
}

pub trait StateExt<E: Into<RpcError>>: SyncTransport<E> + Clone + Sized {
    fn state(&self) -> StateHandle<Self, E> {
        StateHandle {
            transport: self.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<T, E> StateExt<E> for T
where
    T: SyncTransport<E> + Clone,
    E: Into<RpcError>,
{
}

pub struct StateHandle<T, E> {
    transport: T,
    _phantom: PhantomData<E>,
}

impl<T, E> StateHandle<T, E>
where
    T: SyncTransport<E> + Clone,
    E: Into<RpcError>,
{
    pub fn read<V: Serialize + DeserializeOwned>(&self) -> Result<V, LockError> {
        self.read_key("")
    }

    pub fn try_read<V: Serialize + DeserializeOwned>(&self) -> Result<V, LockError> {
        self.try_read_key("")
    }

    pub fn read_or<V: Serialize + DeserializeOwned>(
        &self,
        default: impl FnOnce() -> V,
    ) -> Result<V, LockError> {
        self.read_key_or("", default)
    }

    /// Discouraged, when updating state unless you're certain you want to overwrite the entire state,
    /// use `lock` instead.
    pub fn write<V: Serialize + DeserializeOwned>(&self, value: V) -> Result<(), LockError> {
        self.write_key("", value)
    }

    pub fn lock<V: Serialize + DeserializeOwned + Default>(
        &self,
    ) -> Result<LockedState<T, V, E>, LockError> {
        self.lock_key("")
    }

    pub fn try_lock<V: Serialize + DeserializeOwned>(
        &self,
    ) -> Result<LockedState<T, V, E>, LockError> {
        self.try_lock_key("")
    }

    pub fn lock_or<V: Serialize + DeserializeOwned>(
        &self,
        default: impl FnOnce() -> V,
    ) -> Result<LockedState<T, V, E>, LockError> {
        self.lock_key_or("", default)
    }

    pub fn read_key<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
    ) -> Result<V, LockError> {
        Ok(self.try_lock_key::<V>(key)?.into_inner())
    }

    pub fn try_read_key<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
    ) -> Result<V, LockError> {
        Ok(self.try_lock_key::<V>(key)?.into_inner())
    }

    pub fn read_key_or<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
        default: impl FnOnce() -> V,
    ) -> Result<V, LockError> {
        Ok(self.lock_key_or(key, default)?.into_inner())
    }

    /// Discouraged.  When updating state unless you're certain you want to overwrite the entire state,
    /// use `lock_key` or `lock_key_or` instead.
    pub fn write_key<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
        value: V,
    ) -> Result<(), LockError> {
        let key = key.into();

        //? Lock the key so the host lets us write to it
        let _ = state::LockKey.call(self.transport.clone(), key.clone())?;
        let data = serde_json::to_vec(&value)?;
        state::SetKey.call(self.transport.clone(), (key.clone(), data))??;
        let _ = state::UnlockKey.call(self.transport.clone(), key)?;
        Ok(())
    }

    pub fn lock_key<V: Serialize + DeserializeOwned + Default>(
        &self,
        key: impl Into<String>,
    ) -> Result<LockedState<T, V, E>, LockError> {
        let key = key.into();
        let data = state::LockKey.call(self.transport.clone(), key.clone())?;

        let value: V = if data.is_empty() {
            V::default()
        } else {
            serde_json::from_slice(&data)?
        };
        Ok(LockedState {
            transport: self.transport.clone(),
            key,
            value,
            dirty: false,
            _phantom: PhantomData,
        })
    }

    pub fn try_lock_key<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
    ) -> Result<LockedState<T, V, E>, LockError> {
        let key = key.into();
        let data = state::LockKey.call(self.transport.clone(), key.clone())?;
        if data.is_empty() {
            return Err(LockError::Empty);
        }
        let value: V = serde_json::from_slice(&data)?;
        Ok(LockedState {
            transport: self.transport.clone(),
            key,
            value,
            dirty: false,
            _phantom: PhantomData,
        })
    }

    pub fn lock_key_or<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
        default: impl FnOnce() -> V,
    ) -> Result<LockedState<T, V, E>, LockError> {
        let key = key.into();
        let data = state::LockKey.call(self.transport.clone(), key.clone())?;

        let (value, dirty) = if data.is_empty() {
            (default(), true)
        } else {
            (serde_json::from_slice(&data)?, false)
        };
        Ok(LockedState {
            transport: self.transport.clone(),
            key,
            value,
            dirty,
            _phantom: PhantomData,
        })
    }
}

pub struct LockedState<T, V, E>
where
    T: StateExt<E>,
    V: Serialize,
    E: Into<RpcError>,
{
    transport: T,
    key: String,
    value: V,
    dirty: bool,
    _phantom: PhantomData<E>,
}

impl<T, V, E> LockedState<T, V, E>
where
    T: SyncTransport<E> + Clone,
    V: Serialize,
    E: Into<RpcError>,
{
    pub fn into_inner(self) -> V {
        //? Safe because we consume self and prevent double-drop
        let value = unsafe { std::ptr::read(&self.value) };
        std::mem::forget(self);
        value
    }
}

impl<T, V, E> Deref for LockedState<T, V, E>
where
    T: SyncTransport<E> + Clone,
    V: Serialize,
    E: Into<RpcError>,
{
    type Target = V;
    fn deref(&self) -> &V {
        &self.value
    }
}

impl<T, V, E> DerefMut for LockedState<T, V, E>
where
    T: SyncTransport<E> + Clone,
    V: Serialize,
    E: Into<RpcError>,
{
    fn deref_mut(&mut self) -> &mut V {
        self.dirty = true;
        &mut self.value
    }
}

impl<T, V, E> Drop for LockedState<T, V, E>
where
    T: SyncTransport<E> + Clone,
    V: Serialize,
    E: Into<RpcError>,
{
    fn drop(&mut self) {
        if self.dirty {
            if let Ok(data) = serde_json::to_vec(&self.value) {
                let _ = state::SetKey.call(self.transport.clone(), (self.key.clone(), data));
            }
        }
        let _ = state::UnlockKey.call(self.transport.clone(), self.key.clone());
    }
}
