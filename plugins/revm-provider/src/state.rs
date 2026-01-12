//! State key helpers. Centralized so it's easier to ensure each key is
//! consistent and unique.

use revm::primitives::{Address, U256};

pub fn get_main_key(key: &str) -> String {
    format!("{}/main", key)
}

pub fn get_provider_key(key: &str) -> String {
    format!("{}/provider", key)
}

pub fn get_chain_key(key: &str) -> String {
    format!("{}/chain", key)
}

pub fn get_layer_key(key: &str, layer: u64) -> String {
    format!("{}/layer/{}", key, layer)
}

pub fn get_cache_account_key(key: &str, addr: Address) -> String {
    format!("{}/cache/account/{}", key, addr)
}

pub fn get_cache_storage_key(key: &str, addr: Address, slot: U256) -> String {
    format!("{}/cache/storage/{}/{}", key, addr, slot)
}

pub fn get_cache_block_hash_key(key: &str, number: u64) -> String {
    format!("{}/cache/block_hash/{}", key, number)
}

pub fn get_cache_code_by_hash_key(key: &str, code_hash: revm::primitives::B256) -> String {
    format!("{}/cache/code_by_hash/{}", key, code_hash)
}
