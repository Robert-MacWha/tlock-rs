use std::collections::BTreeMap;

use revm::{
    Context, DatabaseRef, ExecuteCommitEvm, ExecuteEvm, MainBuilder, MainContext,
    bytecode::LegacyAnalyzedBytecode,
    context::{
        BlockEnv, TxEnv,
        result::{EVMError, ExecutionResult, TransactionIndexedError},
    },
    database::{BlockId, CacheDB},
    primitives::{B256, U256, keccak256},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tlock_pdk::{
    tlock_api::alloy::{
        eips::BlockNumberOrTag,
        rpc::types::{BlockOverrides, state::StateOverride},
    },
    wasmi_plugin_pdk::rpc_message::RpcError,
};

use crate::provider_snapshot::ChainSnapshot;

/// Represents a simulated blockchain chain with blocks and transactions.
///  
/// Essentially a lightweight wrapper around REVM's `EVM` with block management
/// and state override capabilities.
pub struct Chain<DB: DatabaseRef> {
    db: CacheDB<DB>,

    /// Pending block environment and transactions
    pending: PendingBlock,
    blocks: BTreeMap<u64, SimulatedBlock>,
    block_time: u64,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct PendingBlock {
    pub env: BlockEnv,
    pub parent_hash: B256,
    pub transactions: Vec<TxEnv>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct SimulatedBlock {
    pub env: BlockEnv,
    pub hash: B256,
    pub parent_hash: B256,
    pub results: Vec<ExecutionResult>,
}

#[derive(Error, Debug)]
pub enum ChainError<DB: DatabaseRef> {
    #[error("RPC Error: {0}")]
    Rpc(#[from] RpcError),
    #[error("EVM Execution Error: {0}")]
    Evm(#[from] EVMError<<DB as DatabaseRef>::Error>),
    #[error("Transaction Index Error: {0}")]
    TransactionIndexerror(#[from] TransactionIndexedError<EVMError<<DB as DatabaseRef>::Error>>),

    #[error("Missing Transaction")]
    MissingTransaction,

    #[error("Database Error: {0}")]
    Db(String),
}

impl<DB: DatabaseRef> Chain<DB> {
    pub fn new(db: DB, block_env: BlockEnv, parent_hash: Option<B256>) -> Self {
        Self {
            db: CacheDB::new(db),
            pending: PendingBlock {
                env: block_env,
                parent_hash: parent_hash.unwrap_or(B256::ZERO),
                transactions: Vec::new(),
            },
            blocks: BTreeMap::new(),
            block_time: 12,
        }
    }

    pub fn from_snapshot(db: DB, snapshot: ChainSnapshot) -> Self {
        let mut db = CacheDB::new(db);
        db.cache = snapshot.cache;
        Self {
            db,
            pending: snapshot.pending,
            blocks: snapshot.blocks,
            block_time: snapshot.block_time,
        }
    }

    pub fn snapshot(&self) -> ChainSnapshot {
        ChainSnapshot {
            cache: self.db.cache.clone(),
            pending: self.pending.clone(),
            blocks: self.blocks.clone(),
            block_time: self.block_time,
        }
    }
}

impl<DB: DatabaseRef> Chain<DB> {
    pub fn latest(&self) -> u64 {
        let latest: u64 = self.pending.env.number.saturating_to();
        latest.saturating_sub(1)
    }

    pub fn pending(&self) -> &PendingBlock {
        &self.pending
    }

    pub fn block(&self, number: u64) -> Option<&SimulatedBlock> {
        self.blocks.get(&number)
    }

    pub fn db_ref(&self) -> Box<&dyn DatabaseRef<Error = DB::Error>> {
        Box::new(&self.db)
    }

    /// Calls a transaction against the chain at the specified block, with
    /// optional state and block overrides. The call does not modify the
    /// chain state and any state overrides are temporary for the duration
    /// of the call.
    pub fn call(
        &self,
        tx: TxEnv,
        block_id: BlockId,
        state_override: Option<StateOverride>,
        block_override: Option<BlockOverrides>,
    ) -> Result<ExecutionResult, ChainError<DB>> {
        let block_env = match self.get_blockenv(block_id) {
            Some(env) => env,
            None => return Err(ChainError::Rpc(RpcError::Custom("Invalid Block ID".into()))),
        };

        let block_env = if let Some(overrides) = block_override {
            apply_block_overrides(&block_env, overrides)
        } else {
            block_env
        };

        //? Only clone the DB if we have state overrides to apply
        let mut db = CacheDB::new(&self.db);
        if let Some(overrides) = state_override {
            apply_state_overrides(&mut db, overrides).map_err(|e| ChainError::Db(e.to_string()))?;
        }

        let mut evm = Context::mainnet()
            .with_ref_db(db)
            .with_block(block_env)
            .build_mainnet();
        let result = evm.transact(tx)?;

        Ok(result.result)
    }

    /// Sends a transaction to the chain, mining it into the pending block and
    /// committing the block to the chain. The chain state is updated to reflect
    /// the changes made by the transaction.
    pub fn transact_commit(&mut self, tx: TxEnv) -> Result<ExecutionResult, ChainError<DB>> {
        let tx_idx = self.pending.transactions.len();

        self.pending.transactions.push(tx);
        let txns = self.mine()?;
        let tx = txns
            .into_iter()
            .nth(tx_idx)
            .ok_or(ChainError::MissingTransaction)?;

        Ok(tx)
    }

    /// Mines the pending block, committing it to the chain and advancing
    /// the latest block number.
    ///
    /// TODO: Make public and add `transact` as an alternative to
    /// `transact_commit`
    fn mine(&mut self) -> Result<Vec<ExecutionResult>, ChainError<DB>> {
        let db = &mut self.db;
        let block_env = self.pending.env.clone();
        let mut evm = Context::mainnet()
            .with_ref_db(db)
            .with_block(block_env)
            .build_mainnet();

        let results = evm.transact_many_commit(self.pending.transactions.clone().into_iter())?;

        //? Store simulated
        let block_number: u64 = self.pending.env.number.saturating_to();
        let parent_hash = self.pending.parent_hash;
        let block_hash = self.compute_block_hash(block_number, parent_hash);

        self.blocks.insert(
            block_number,
            SimulatedBlock {
                env: self.pending.env.clone(),
                parent_hash,
                hash: block_hash,
                results: results.clone(),
            },
        );
        self.db
            .cache
            .block_hashes
            .insert(U256::from(block_number), block_hash);

        //? Advance to next block, updating pending
        let latest = block_number + 1;
        self.pending = PendingBlock {
            transactions: Vec::new(),
            parent_hash: block_hash,
            env: BlockEnv {
                number: U256::from(latest),
                beneficiary: self.pending.env.beneficiary,
                timestamp: self
                    .pending
                    .env
                    .timestamp
                    .saturating_add(U256::from(self.block_time)),
                gas_limit: self.pending.env.gas_limit,
                basefee: self.pending.env.basefee,
                difficulty: self.pending.env.difficulty,
                prevrandao: self.pending.env.prevrandao,
                blob_excess_gas_and_price: self.pending.env.blob_excess_gas_and_price,
            },
        };

        Ok(results)
    }

    fn get_blockenv(&self, block: BlockId) -> Option<BlockEnv> {
        // TODO: Add cases for remaining tags (finalized, safe, earliest) and hashes
        match block {
            BlockId::Number(BlockNumberOrTag::Pending) => Some(self.pending.env.clone()),
            BlockId::Number(BlockNumberOrTag::Latest) => {
                self.blocks.get(&self.latest()).map(|b| b.env.clone())
            }
            BlockId::Number(BlockNumberOrTag::Number(n)) => {
                self.blocks.get(&n).map(|b| b.env.clone())
            }
            _ => None,
        }
    }

    fn compute_block_hash(&self, number: u64, parent_hash: B256) -> B256 {
        keccak256([number.to_be_bytes().as_slice(), parent_hash.as_slice()].concat())
    }
}

fn apply_state_overrides<DB: DatabaseRef>(
    db: &mut CacheDB<DB>,
    overrides: StateOverride,
) -> Result<(), DB::Error> {
    for (address, account_override) in overrides.iter() {
        let db_account = db.load_account(*address)?;

        db_account.info.balance = account_override.balance.unwrap_or(db_account.info.balance);
        db_account.info.nonce = account_override.nonce.unwrap_or(db_account.info.nonce);

        if let Some(code) = &account_override.code {
            db_account
                .info
                .set_code(revm::state::Bytecode::LegacyAnalyzed(
                    LegacyAnalyzedBytecode::analyze(code.clone()),
                ));
        }

        //? State replaces all existing
        if let Some(state) = &account_override.state {
            db_account.storage.clear();
            for (slot, value) in state.iter() {
                db_account.storage.insert((*slot).into(), (*value).into());
            }
        }

        //? State diffs merge with existing
        if let Some(state_diff) = &account_override.state_diff {
            for (slot, value) in state_diff.iter() {
                db_account.storage.insert((*slot).into(), (*value).into());
            }
        }
    }

    Ok(())
}

fn apply_block_overrides(block_env: &BlockEnv, overrides: BlockOverrides) -> BlockEnv {
    BlockEnv {
        number: overrides.number.unwrap_or(block_env.number),
        beneficiary: overrides.coinbase.unwrap_or(block_env.beneficiary),
        gas_limit: overrides.gas_limit.unwrap_or(block_env.gas_limit),
        prevrandao: overrides.random,
        difficulty: overrides.difficulty.unwrap_or(block_env.difficulty),
        blob_excess_gas_and_price: block_env.blob_excess_gas_and_price,
        timestamp: overrides
            .time
            .map(|n| U256::from(n))
            .unwrap_or(block_env.timestamp),
        basefee: overrides
            .base_fee
            .map(|n| n.saturating_to())
            .unwrap_or(block_env.basefee),
    }
}
