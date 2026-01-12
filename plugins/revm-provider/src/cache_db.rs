use revm::{
    DatabaseRef,
    context::DBErrorMarker,
    primitives::{Address, B256},
    state::{AccountInfo, Bytecode},
};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tlock_pdk::{
    state::{self, StateExt},
    tlock_api::alloy::network::Network,
    wasmi_plugin_pdk::transport::Transport,
};

use crate::state::{
    get_cache_account_key, get_cache_block_hash_key, get_cache_code_by_hash_key,
    get_cache_storage_key,
};

/// A caching database that wraps an underlying read-only database and caches
/// all reads using the transport's state storage.
#[derive(Debug)]
pub struct CacheDB<ExtDB, N: Network> {
    transport: Transport,
    /// Cache key prefix
    key: String,

    /// Read-only underlying database
    pub db: ExtDB,
    _marker: core::marker::PhantomData<fn() -> N>,
}

#[derive(Debug, Error)]
pub enum CacheDBError<ExtDBError> {
    #[error("ExtDBError: {0}")]
    ExtDBError(ExtDBError),
    #[error("State lock error: {0}")]
    StateLockError(state::LockError),
}

impl<E> DBErrorMarker for CacheDBError<E> {}

impl<ExtDB, N: Network> CacheDB<ExtDB, N> {
    pub fn new(transport: Transport, key: String, db: ExtDB) -> Self {
        Self {
            transport,
            key,
            db,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<ExtDB: DatabaseRef, N: Network> DatabaseRef for CacheDB<ExtDB, N> {
    type Error = CacheDBError<ExtDB::Error>;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let cache_key = get_cache_account_key(&self.key, address);
        self.cached_or_fetch(cache_key, || self.db.basic_ref(address))
    }

    fn storage_ref(
        &self,
        address: Address,
        index: revm::primitives::StorageKey,
    ) -> Result<revm::primitives::StorageValue, Self::Error> {
        let cache_key = get_cache_storage_key(&self.key, address, index);
        self.cached_or_fetch(cache_key, || self.db.storage_ref(address, index))
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        let cache_key = get_cache_block_hash_key(&self.key, number);
        self.cached_or_fetch(cache_key, || self.db.block_hash_ref(number))
    }

    fn code_by_hash_ref(&self, code: B256) -> Result<Bytecode, Self::Error> {
        let cache_key = get_cache_code_by_hash_key(&self.key, code);
        self.cached_or_fetch(cache_key, || self.db.code_by_hash_ref(code))
    }
}

impl<ExtDB: DatabaseRef, N: Network> CacheDB<ExtDB, N> {
    fn cached_or_fetch<T, F>(
        &self,
        cache_key: String,
        fetch: F,
    ) -> Result<T, CacheDBError<ExtDB::Error>>
    where
        T: Serialize + DeserializeOwned + Clone + 'static,
        F: FnOnce() -> Result<T, ExtDB::Error>,
    {
        // Attempt to lock the cache key. If it exists, return the cached value and
        // otherwise get the value from the underlying DB and store it in cache.
        match self.transport.state().try_lock_key::<T>(cache_key.clone()) {
            Ok(cached) => Ok(cached.into_inner()),
            Err(state::LockError::Empty(cache_key)) => {
                let value = fetch().map_err(CacheDBError::ExtDBError)?;
                //? Small race condition if multiple callers try to fetch the same key at once,
                //? but won't lead to incorrect results, just redundant fetches.
                self.transport
                    .state()
                    .write_key(cache_key, value.clone())
                    .map_err(CacheDBError::StateLockError)?;

                Ok(value)
            }
            Err(e) => Err(CacheDBError::StateLockError(e)),
        }
    }
}
