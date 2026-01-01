use std::collections::BTreeMap;

use revm::{
    database::Cache,
    primitives::{HashMap, alloy_primitives::TxHash},
};
use serde::{Deserialize, Serialize};
use tlock_pdk::tlock_api::alloy::rpc::{self};

use crate::chain::{PendingBlock, SimulatedBlock};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProviderSnapshot {
    pub chain: ChainSnapshot,
    pub transactions: HashMap<u64, Vec<rpc::types::Transaction>>,
    pub receipts: HashMap<TxHash, rpc::types::TransactionReceipt>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ChainSnapshot {
    pub cache: Cache,
    pub pending: PendingBlock,
    pub blocks: BTreeMap<u64, SimulatedBlock>,
    pub block_time: u64,
}
