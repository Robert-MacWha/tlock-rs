use std::collections::BTreeMap;

use revm::{
    Context, DatabaseRef, ExecuteCommitEvm, ExecuteEvm, MainBuilder, MainContext,
    bytecode::LegacyAnalyzedBytecode,
    context::{
        BlockEnv, TxEnv,
        result::{EVMError, ExecutionResult, TransactionIndexedError},
    },
    database::CacheDB,
    primitives::{Address, B256, U256, address, keccak256},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tlock_pdk::{
    tlock_api::alloy::{
        eips::{BlockId, BlockNumberOrTag},
        rpc::types::{BlockOverrides, state::StateOverride},
    },
    wasmi_plugin_pdk::rpc_message::RpcError,
};
use tracing::info;

use crate::provider_snapshot::ChainSnapshot;

/// Represents a simulated blockchain chain with blocks and transactions.
///  
/// Essentially a lightweight wrapper around REVM's `EVM` with block management
/// and state override capabilities.
pub struct Chain<DB: DatabaseRef> {
    db: CacheDB<DB>,

    /// Pending block environment and transactions
    pending: PendingBlock,
    chain_id: u64,
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
    pub transactions: Vec<TxEnv>,
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

impl<DB: DatabaseRef + std::fmt::Debug> Chain<DB> {
    pub fn new(db: DB, chain_id: u64, block_env: BlockEnv, parent_hash: Option<B256>) -> Self {
        let mut chain = Self {
            db: CacheDB::new(db),
            chain_id,
            pending: PendingBlock {
                env: block_env,
                parent_hash: parent_hash.unwrap_or(B256::ZERO),
                transactions: Vec::new(),
            },
            blocks: BTreeMap::new(),
            block_time: 12,
        };
        //? Safe because we start with no transactions
        chain.mine().unwrap();
        chain
    }

    pub fn from_snapshot(db: DB, snapshot: ChainSnapshot) -> Self {
        let mut db = CacheDB::new(db);
        db.cache = snapshot.cache;

        Self {
            db,
            chain_id: snapshot.chain_id,
            pending: snapshot.pending,
            blocks: snapshot.blocks,
            block_time: snapshot.block_time,
        }
    }

    pub fn snapshot(&self) -> ChainSnapshot {
        ChainSnapshot {
            cache: self.db.cache.clone(),
            chain_id: self.chain_id,
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

    pub fn db(&mut self) -> &mut CacheDB<DB> {
        &mut self.db
    }

    /// Calls a transaction against the chain at the specified block, with
    /// optional state and block overrides. The call does not modify the
    /// chain state and any state overrides are temporary for the duration
    /// of the call.
    ///
    /// The `unconstrained` flag, when set to true, allows the call to bypass
    /// certain constraints such as gas limits and balance checks.
    pub fn call(
        &mut self,
        mut tx: TxEnv,
        block_id: BlockId,
        state_override: Option<StateOverride>,
        block_override: Option<BlockOverrides>,
        unconstrained: bool,
    ) -> Result<ExecutionResult, ChainError<DB>> {
        info!("eth_call {:?} at block {:?}", tx, block_id);

        let mut block_env = match self.get_blockenv(block_id) {
            Some(env) => env,
            None => return Err(ChainError::Rpc(RpcError::Custom("Invalid Block ID".into()))),
        };
        block_env.basefee = 0;

        let block_env = if let Some(overrides) = block_override {
            apply_block_overrides(&block_env, overrides)
        } else {
            block_env
        };

        //? Stack a new CacheDB to apply state overrides without modifying
        //? the underlying chain state
        let mut overlay_db = CacheDB::new(&self.db);
        if let Some(overrides) = state_override {
            apply_state_overrides(&mut overlay_db, overrides)
                .map_err(|e| ChainError::Db(e.to_string()))?;
        }

        if unconstrained {
            tx.gas_price = 0;
            tx.gas_priority_fee = None;
            tx.gas_limit = u64::MAX;
        }

        if unconstrained && tx.caller == Address::ZERO {
            tx.caller = address!("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
        }

        let mut evm = Context::mainnet()
            .modify_cfg_chained(|cfg| {
                cfg.tx_chain_id_check = false;
                cfg.chain_id = self.chain_id;
                cfg.disable_nonce_check = true;
            })
            .with_db(overlay_db)
            .with_block(block_env)
            .build_mainnet();

        if unconstrained {
            evm.cfg.disable_balance_check = true;
            evm.cfg.disable_base_fee = true;
            evm.cfg.disable_block_gas_limit = true;
        }

        let result = evm.transact(tx)?;

        Ok(result.result)
    }

    /// Sends a transaction to the chain, mining it into the pending block and
    /// committing the block to the chain. The chain state is updated to reflect
    /// the changes made by the transaction.
    pub fn transact_commit(&mut self, tx: TxEnv) -> Result<ExecutionResult, ChainError<DB>> {
        info!(
            "Transacting tx {:?} at block {:?}",
            tx, self.pending.env.number
        );
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
    pub fn mine(&mut self) -> Result<Vec<ExecutionResult>, ChainError<DB>> {
        let db = &mut self.db;
        let block_env = self.pending.env.clone();
        let mut evm = Context::mainnet()
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = self.chain_id;
            })
            .with_db(db)
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
                transactions: self.pending.transactions.clone(),
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
