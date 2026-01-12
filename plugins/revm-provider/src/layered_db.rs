use revm::{
    DatabaseRef,
    primitives::{Address, B256},
    state::{AccountInfo, Bytecode, EvmState},
};
use serde::{Deserialize, Serialize};
use tlock_pdk::tlock_api::alloy::network::Network;

/// A layered database that maintains multiple layers of EVM state stacked
/// on top of each other. Reads first check the layers in order before falling
/// back to the underlying db.
#[derive(Debug)]
pub struct LayeredDB<ExtDB, N: Network> {
    layers: Vec<LayerState>,

    /// Read-only underlying database
    db: ExtDB,
    _marker: core::marker::PhantomData<fn() -> N>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LayerState {
    pub block_number: u64,
    pub block_hash: B256,
    pub state: EvmState,
}

impl<ExtDB, N: Network> LayeredDB<ExtDB, N> {
    pub fn new(db: ExtDB) -> Self {
        Self {
            layers: vec![],
            db,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn push_layer(&mut self, layer: LayerState) {
        self.layers.push(layer);
    }
}

impl<ExtDB: DatabaseRef, N: Network> DatabaseRef for LayeredDB<ExtDB, N> {
    type Error = ExtDB::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        for layer in self.layers.iter().rev() {
            if let Some(account) = layer.state.get(&address) {
                //? Only return if the account is not default. Otherwise continue to check lower layers.
                if account.info != AccountInfo::default() {
                    return Ok(Some(account.info.clone()));
                }
            }
        }

        return self.db.basic_ref(address);
    }

    fn storage_ref(
        &self,
        address: Address,
        index: revm::primitives::StorageKey,
    ) -> Result<revm::primitives::StorageValue, Self::Error> {
        for layer in self.layers.iter().rev() {
            let Some(account) = layer.state.get(&address) else {
                continue;
            };

            if let Some(value) = account.storage.get(&index) {
                return Ok(value.present_value());
            }
        }

        self.db.storage_ref(address, index)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        for layer in self.layers.iter().rev() {
            if number == layer.block_number {
                return Ok(layer.block_hash);
            }
        }

        self.db.block_hash_ref(number)
    }

    fn code_by_hash_ref(&self, code: B256) -> Result<Bytecode, Self::Error> {
        // TODO: If this becomes a performance bottleneck, consider caching code in a HashMap
        for layer in self.layers.iter().rev() {
            for account in layer.state.values() {
                if account.info.code_hash == code
                    && let Some(code) = &account.info.code
                {
                    return Ok(code.clone());
                }
            }
        }

        self.db.code_by_hash_ref(code)
    }
}
