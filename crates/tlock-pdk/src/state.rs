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
use tracing::error;
use wasmi_plugin_pdk::{rpc_message::RpcError, transport::SyncTransport};

#[derive(Debug, Error)]
pub enum LockError {
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Key is empty: {0}")]
    Empty(String),
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

pub struct LockedState<T, V, E>
where
    T: StateExt<E>,
    V: Serialize,
    E: Into<RpcError>,
{
    guard: LockGuard<T, E>,
    value: Option<V>, // Only None after into_inner
    dirty: bool,
}

pub struct LockGuard<T, E>
where
    T: SyncTransport<E> + Clone,
    E: Into<RpcError>,
{
    transport: T,
    key: String,
    _phantom: PhantomData<E>,
}

// TODO: Change API to follow mutex-style locking as closely as possible, with the
// addition of keys of course.
impl<T, E> StateHandle<T, E>
where
    T: SyncTransport<E> + Clone,
    E: Into<RpcError>,
{
    pub fn read<V: Serialize + DeserializeOwned>(&self) -> Result<V, LockError> {
        self.read_key("")
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
        let key = key.into();
        let data = state::ReadKey.call(self.transport.clone(), key.clone())?;
        if data.is_empty() {
            return Err(LockError::Empty(key));
        }
        let value: V = serde_json::from_slice(&data)?;
        Ok(value)
    }

    /// Read the state at `key` or return the result of `default` if the state is empty.
    ///
    /// Does not write back to state if the key is empty.
    pub fn read_key_or<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
        default: impl FnOnce() -> V,
    ) -> Result<V, LockError> {
        let key = key.into();
        let data = state::ReadKey.call(self.transport.clone(), key.clone())?;
        if data.is_empty() {
            return Ok(default());
        }
        let value: V = serde_json::from_slice(&data)?;
        Ok(value)
    }

    /// Writes the entire state at `key`.
    ///
    /// Discouraged. Use `lock_key` to modify the state unless you're certain you
    /// want to overwrite the entire state. NEVER write a value you previously
    /// read since other instances may have modified the state in the meantime.
    ///
    /// TODO: Consider some kind of global state + warning if someone writes to
    /// a key after reading from it.
    pub fn write_key<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
        value: V,
    ) -> Result<(), LockError> {
        let key = key.into();

        //? Lock the key so the host lets us write to it
        let (_guard, _data) = LockGuard::acquire(self.transport.clone(), key.clone())?;
        let data = serde_json::to_vec(&value)?;
        state::SetKey.call(self.transport.clone(), (key.clone(), data))??;
        let _ = state::UnlockKey.call(self.transport.clone(), key)?;
        Ok(())
    }

    /// Lock the key, initializing with default if empty.
    pub fn lock_key<V: Serialize + DeserializeOwned + Default>(
        &self,
        key: impl Into<String>,
    ) -> Result<LockedState<T, V, E>, LockError> {
        let key = key.into();
        let (guard, data) = LockGuard::acquire(self.transport.clone(), key.clone())?;

        let value: V = if data.is_empty() {
            V::default()
        } else {
            serde_json::from_slice(&data)?
        };

        Ok(LockedState {
            guard,
            value: Some(value),
            dirty: false,
        })
    }

    /// Try to lock the key, returning an error if the key is empty.
    pub fn try_lock_key<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
    ) -> Result<LockedState<T, V, E>, LockError> {
        let key = key.into();
        let (guard, data) = LockGuard::acquire(self.transport.clone(), key.clone())?;

        if data.is_empty() {
            return Err(LockError::Empty(key));
        }

        let value: V = serde_json::from_slice(&data)?;

        Ok(LockedState {
            guard,
            value: Some(value),
            dirty: false,
        })
    }

    /// Try to lock the key, returning the result of `default` if the key is empty.
    pub fn lock_key_or<V: Serialize + DeserializeOwned>(
        &self,
        key: impl Into<String>,
        default: impl FnOnce() -> V,
    ) -> Result<LockedState<T, V, E>, LockError> {
        let key = key.into();
        let (guard, data) = LockGuard::acquire(self.transport.clone(), key.clone())?;

        let (value, dirty) = if data.is_empty() {
            (default(), true)
        } else {
            let value: V = serde_json::from_slice(&data)?;
            (value, false)
        };

        Ok(LockedState {
            guard,
            value: Some(value),
            dirty,
        })
    }
}

impl<T, V, E> LockedState<T, V, E>
where
    T: SyncTransport<E> + Clone,
    V: Serialize,
    E: Into<RpcError>,
{
    pub fn into_inner(mut self) -> V {
        self.dirty = false;
        //? Safe because we only take it on consuming self
        self.value.take().expect("value already taken")
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
        self.value.as_ref().expect("value already taken")
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
        self.value.as_mut().expect("value already taken")
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
            if let Some(ref value) = self.value {
                if let Ok(data) = serde_json::to_vec(value) {
                    self.guard.set(data);
                }
            }
        }
        // Unlocked when guard is dropped
    }
}

impl<T, E> LockGuard<T, E>
where
    T: SyncTransport<E> + Clone,
    E: Into<RpcError>,
{
    pub fn acquire(transport: T, key: String) -> Result<(Self, Vec<u8>), LockError> {
        let data = state::LockKey.call(transport.clone(), key.clone())?;
        Ok((
            Self {
                transport,
                key,
                _phantom: PhantomData,
            },
            data,
        ))
    }

    pub fn set(&self, data: Vec<u8>) {
        let result = state::SetKey.call(self.transport.clone(), (self.key.clone(), data));
        let result = match result {
            Ok(res) => res,
            Err(err) => {
                error!("Failed to set key '{}': {}", self.key, err);
                return;
            }
        };

        match result {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to set key '{}': {}", self.key, err);
                return;
            }
        };
    }
}

impl<T, E> Drop for LockGuard<T, E>
where
    T: SyncTransport<E> + Clone,
    E: Into<RpcError>,
{
    fn drop(&mut self) {
        let _ = state::UnlockKey.call(self.transport.clone(), self.key.clone());
    }
}
