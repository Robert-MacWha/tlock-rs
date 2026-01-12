use std::collections::BTreeMap;

use erc20s::{ERC20S, get_erc20_by_address};
use revm::{
    Context, DatabaseRef, ExecuteEvm, MainBuilder, MainContext,
    bytecode::LegacyAnalyzedBytecode,
    context::{BlockEnv, TxEnv, result::ExecutionResult},
    database::{CacheDB as RevmCacheDB, WrapDatabaseRef},
    interpreter::instructions::utility::IntoU256,
    primitives::{Address, B256, U256, address, keccak256},
    state::{EvmState, EvmStorageSlot},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tlock_pdk::{
    state::{LockedState, StateExt},
    tlock_api::alloy::{
        eips::{BlockId, BlockNumberOrTag},
        network::{Ethereum, Network},
        rpc::{
            self,
            types::{BlockOverrides, state::StateOverride},
        },
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, RpcErrorContext, ToRpcResult},
        transport::{Transport, TransportError},
    },
};
use tracing::{error, info, warn};

use crate::{
    cache_db::{CacheDB, CacheDBError},
    layered_db::{LayerState, LayeredDB},
    remote_db::{AlloyDBError, RemoteDB},
    rpc::header_to_block_env,
    state::{get_chain_key, get_layer_key},
};

/// Represents a forked execution chain for simulating Ethereum transactions.
/// Maintains the chain state, including blocks, pending transactions, and the
/// layered database.
pub struct Chain {
    transport: Transport,
    key: String,
    fork_url: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChainState {
    chain_id: u64,
    /// Block at which the chain was forked. This block is static and fetched
    /// from remote and all subsequent blocks are simulated locally.
    fork_block_number: u64,
    block_time: u64,
    pending: PendingBlock,
    blocks: BTreeMap<u64, SimulatedBlock>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct PendingBlock {
    pub env: BlockEnv,
    pub parent_hash: B256,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct SimulatedBlock {
    pub env: BlockEnv,
    pub hash: B256,
    pub parent_hash: B256,
    pub results: Vec<ExecutionResult>,
}

#[derive(Error, Debug)]
pub enum ChainError {
    #[error("RPC Error: {0}")]
    Rpc(#[from] RpcError),
    #[error("EVM Execution Error: {0}")]
    Evm(String),
    #[error("Transaction Index Error: {0}")]
    TransactionIndexError(String),

    #[error("Missing Transaction")]
    MissingTransaction,

    #[error("Database Error: {0}")]
    Db(String),
}

impl Chain {
    /// Creates a new forked chain instance with the provided parameters.
    pub fn new(
        transport: Transport,
        key: String,
        fork_url: String,
        header: rpc::types::Header,
        chain_id: u64,
        block_time: u64,
    ) -> Result<Self, ChainError> {
        let parent_hash = header.parent_hash;
        let block_env = header_to_block_env(header);

        //? The fork block is the parent of the provided header. This is because
        //? we use the provided as the base for the pending block so we
        //? don't need to construct it ourselves.
        let fork_block_number: u64 = block_env.number.saturating_to();
        let fork_block_number = fork_block_number.saturating_sub(1);
        let chain_state = ChainState {
            chain_id,
            block_time,
            fork_block_number,
            pending: PendingBlock {
                env: block_env,
                parent_hash,
            },
            blocks: BTreeMap::new(),
        };

        let state_key = get_chain_key(&key);
        let mut state = transport
            .state()
            .lock_key_or(state_key, || chain_state.clone())
            .rpc_err()?;
        *state = chain_state;

        let chain = Self {
            transport,
            key,
            fork_url,
        };

        //? Mine an initial empty block to establish the fork state
        chain.mine_with_state(&mut state, EvmState::default(), vec![])?;

        Ok(chain)
    }

    /// Load an existing chain from storage.
    pub fn load(transport: Transport, key: String, fork_url: String) -> Self {
        Self {
            transport,
            key,
            fork_url,
        }
    }
}

impl Chain {
    /// Retrieves a clone of the current chain state. Changes made to the returned
    /// value do not affect the actual chain state. To modify the chain state,
    /// lock it directly.
    fn clone_state(&self) -> Result<ChainState, ChainError> {
        let state_key = get_chain_key(&self.key);
        let state: ChainState = self.transport.state().try_read_key(&state_key).rpc_err()?;
        Ok(state)
    }

    pub fn latest(&self) -> Result<u64, ChainError> {
        let state = self.clone_state()?;
        let latest: u64 = state.pending.env.number.saturating_to();
        Ok(latest.saturating_sub(1))
    }

    pub fn pending(&self) -> Result<PendingBlock, ChainError> {
        let state = self.clone_state()?;
        Ok(state.pending)
    }

    pub fn block(&self, number: u64) -> Result<Option<SimulatedBlock>, ChainError> {
        //? Mutable borrow to avoid cloning the block. Don't want to actually
        //? persist the change, so using `get_state` is fine.
        let mut state = self.clone_state()?;
        Ok(state.blocks.remove(&number))
    }

    /// Returns a snapshot of the database at the requested block number.
    pub fn db(
        &self,
        block_id: BlockId,
    ) -> Result<Box<dyn DatabaseRef<Error = CacheDBError<AlloyDBError>>>, ChainError> {
        let state = self.clone_state()?;
        let number = block_id_to_number(&state, &block_id).context("Invalid Block ID")?;

        let db = construct_db::<Ethereum>(
            self.transport.clone(),
            self.key.clone(),
            self.fork_url.clone(),
            state.fork_block_number,
            number,
        )?;

        Ok(db)
    }

    /// Calls a transaction against the chain at the specified block, with
    /// optional state and block overrides. The call does not modify the
    /// chain state and any state overrides are temporary for the duration
    /// of the call.
    ///
    /// The `unconstrained` flag, when set to true, allows the call to bypass
    /// certain constraints such as gas limits and balance checks.
    pub fn call(
        &self,
        mut tx: TxEnv,
        block_id: BlockId,
        state_override: Option<StateOverride>,
        block_override: Option<BlockOverrides>,
        unconstrained: bool,
    ) -> Result<ExecutionResult, ChainError> {
        let state = self.clone_state()?;

        let mut block_env = match get_blockenv(&state, &block_id) {
            Some(env) => env,
            None => {
                return Err(ChainError::Rpc(RpcError::custom(format!(
                    "Invalid Block ID: {}",
                    block_id
                ))));
            }
        };
        block_env.basefee = 0; //? Disable basefee for calls

        let block_env = if let Some(overrides) = block_override {
            apply_block_overrides(&block_env, overrides)
        } else {
            block_env
        };

        //? Stack a new CacheDB to apply state overrides without modifying
        //? the underlying chain state
        let latest: u64 = state.pending.env.number.saturating_to();
        let latest = latest.saturating_sub(1);
        let db = construct_db::<Ethereum>(
            self.transport.clone(),
            self.key.clone(),
            self.fork_url.clone(),
            state.fork_block_number,
            latest,
        )?;
        let mut overlay_db = RevmCacheDB::new(db);
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
                cfg.disable_nonce_check = true;
                cfg.chain_id = state.chain_id;
            })
            .with_db(overlay_db)
            .with_block(block_env)
            .build_mainnet();

        if unconstrained {
            evm.cfg.disable_balance_check = true;
            evm.cfg.disable_base_fee = true;
            evm.cfg.disable_block_gas_limit = true;
        }

        let result = evm
            .transact(tx)
            .map_err(|e| ChainError::Evm(e.to_string()))?;
        Ok(result.result)
    }

    /// Mines a block containing the transaction and updates the chain state.
    pub fn transact_commit(&self, tx: TxEnv) -> Result<ExecutionResult, ChainError> {
        //? Inspect the erc20 balances of the sender before executing the tx
        for erc20 in ERC20S.iter() {
            let balance = self.inspect_erc20(tx.caller, erc20.address)?;
            info!(
                "Pre-Tx Balance of {} for {:?}: {}",
                erc20.symbol, tx.caller, balance
            );
        }

        let state_key = get_chain_key(&self.key);
        let mut state = self
            .transport
            .state()
            .try_lock_key::<ChainState>(state_key)
            .rpc_err()?;

        //* Prepare the EVM with the current chain state
        let latest: u64 = state.pending.env.number.saturating_to();
        let latest = latest.saturating_sub(1);
        let db = construct_db::<Ethereum>(
            self.transport.clone(),
            self.key.clone(),
            self.fork_url.clone(),
            state.fork_block_number,
            latest,
        )?;
        let db = WrapDatabaseRef::from(db);

        let mut evm = Context::mainnet()
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = state.chain_id;
            })
            .with_db(db)
            .with_block(state.pending.env.clone())
            .build_mainnet();

        //* Execute the provided transaction
        let transactions = vec![tx];
        let exec_result_and_state = evm
            .transact_many_finalize(transactions.clone().into_iter())
            .map_err(|e| ChainError::TransactionIndexError(e.to_string()))?;

        let exec_results = exec_result_and_state.result;
        //? Safe since we provide one tx in `transact_many_finalize`
        let exec_result = exec_results
            .get(0)
            .context("Missing transaction in ExecutionResult")?
            .clone();

        match exec_result {
            ExecutionResult::Success { .. } => {}
            ExecutionResult::Halt {
                ref reason,
                gas_used,
            } => {
                error!(
                    "Transaction halted. Gas used: {}. Reason: {:?}",
                    gas_used, reason
                );
            }
            ExecutionResult::Revert {
                gas_used,
                ref output,
            } => {
                let reason = if output.is_empty() {
                    "Empty revert (no message)".to_string()
                } else if output.len() >= 4 && output[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
                    String::from_utf8_lossy(&output[68..])
                        .trim_matches(char::from(0))
                        .to_string()
                } else {
                    format!("Raw Hex: {:?}", output)
                };

                error!(
                    "Transaction reverted. Gas used: {}. Reason: {}",
                    gas_used, reason
                );
            }
        };

        info!(
            "Committed transaction in block {}: {:?}",
            state.pending.env.number, exec_result
        );

        let exec_state = exec_result_and_state.state;

        //* Update the chain state with the new block, transaction result, and state.
        self.mine_with_state(&mut state, exec_state, exec_results)?;

        Ok(exec_result)
    }

    /// Internal helper to advance the state.
    fn mine_with_state(
        &self,
        state: &mut LockedState<Transport, ChainState, TransportError>,
        evm_state: revm::state::EvmState,
        exec_results: Vec<ExecutionResult>,
    ) -> Result<(), ChainError> {
        //? Store the new block
        let new_block_number: u64 = state.pending.env.number.saturating_to();
        let block_time = state.block_time;
        let new_parent_hash = state.pending.parent_hash;
        let new_block_hash = compute_block_hash(new_block_number, new_parent_hash);
        let new_pending = state.pending.clone();

        info!("Mining block {}", new_block_number);

        state.blocks.insert(
            new_block_number,
            SimulatedBlock {
                env: new_pending.env.clone(),
                parent_hash: new_parent_hash,
                hash: new_block_hash,
                results: exec_results,
            },
        );

        //? Create new layer state for the committed block
        let layer_state = LayerState {
            block_number: new_block_number,
            block_hash: new_block_hash,
            state: evm_state,
        };
        let layer_key = get_layer_key(&self.key, new_block_number);
        self.transport
            .state()
            .write_key(layer_key, layer_state)
            .rpc_err()?;

        //? Advance the pending block
        let new_pending_number = new_block_number + 1;
        state.pending = PendingBlock {
            parent_hash: new_block_hash,
            env: BlockEnv {
                number: U256::from(new_pending_number),
                beneficiary: new_pending.env.beneficiary,
                timestamp: new_pending
                    .env
                    .timestamp
                    .saturating_add(U256::from(block_time)),
                gas_limit: new_pending.env.gas_limit,
                basefee: new_pending.env.basefee,
                difficulty: new_pending.env.difficulty,
                prevrandao: new_pending.env.prevrandao,
                blob_excess_gas_and_price: new_pending.env.blob_excess_gas_and_price,
            },
        };

        Ok(())
    }
}

// ---------- CHEATCODES ----------
// Cheatcodes directly modify the chain state. They do this by "mining" a new block
// with the modified state.

#[allow(dead_code)]
impl Chain {
    /// Mine a new block with no state changes.
    pub fn mine(&self) -> Result<(), ChainError> {
        info!("mine");

        let state_key = get_chain_key(&self.key);
        let mut state = self
            .transport
            .state()
            .try_lock_key::<ChainState>(state_key)
            .rpc_err()?;

        //? No state changes, just advance the block
        let evm_state = EvmState::default();
        self.mine_with_state(&mut state, evm_state, vec![])
    }

    /// Sets the balance of the specified address by mining a new block
    pub fn deal(&self, address: Address, amount: U256) -> Result<(), ChainError> {
        let state_key = get_chain_key(&self.key);
        let mut state = self
            .transport
            .state()
            .try_lock_key::<ChainState>(state_key)
            .rpc_err()?;

        let mut evm_state = EvmState::default();
        evm_state.entry(address).or_default().info.balance = amount;

        self.mine_with_state(&mut state, evm_state, vec![])
    }

    /// Sets the ERC20 token balance of the specified address by mining a new block
    ///
    /// Because ERC20 balances are stored at different storage slots depending on the token,
    /// this isn't guaranteed to work for unknown tokens.
    pub fn deal_erc20(
        &self,
        address: Address,
        token: Address,
        amount: U256,
    ) -> Result<(), ChainError> {
        let state_key = get_chain_key(&self.key);
        let mut state = self
            .transport
            .state()
            .try_lock_key::<ChainState>(state_key)
            .rpc_err()?;

        let slot = if let Some(erc20) = get_erc20_by_address(&token) {
            erc20.slot
        } else {
            warn!("Attempting to deal ERC20 for unknown token: {:?}", token);
            0
        };

        // Storage key = keccak256(abi.encode(holder, slot))
        // holder is left-padded to 32 bytes, then slot as 32 bytes
        let mut key_preimage = [0u8; 64];
        key_preimage[12..32].copy_from_slice(address.as_slice()); // address at bytes 12-31
        key_preimage[32..64].copy_from_slice(&U256::from(slot).to_be_bytes::<32>()); // slot at bytes 32-63

        let storage_key = keccak256(key_preimage);
        let storage_key = storage_key.into_u256();

        //? Set the storage slot to the desired amount. transaction_id is unused
        //? since we're setting the value directly.
        let mut evm_state = EvmState::default();
        *evm_state
            .entry(token)
            .or_default()
            .storage
            .entry(storage_key)
            .or_default() = EvmStorageSlot::new(amount, 0);

        self.mine_with_state(&mut state, evm_state, vec![])
    }

    /// Inspects the ERC20 token balance of the specified address.
    fn inspect_erc20(&self, address: Address, token: Address) -> Result<U256, ChainError> {
        let state = self.clone_state()?;

        let latest: u64 = state.pending.env.number.saturating_to();
        let latest = latest.saturating_sub(1);
        let db = construct_db::<Ethereum>(
            self.transport.clone(),
            self.key.clone(),
            self.fork_url.clone(),
            state.fork_block_number,
            latest,
        )?;

        let slot = if let Some(erc20) = get_erc20_by_address(&token) {
            erc20.slot
        } else {
            warn!("Attempting to inspect ERC20 for unknown token: {:?}", token);
            0
        };

        // Storage key = keccak256(abi.encode(holder, slot))
        // holder is left-padded to 32 bytes, then slot as 32 bytes
        let mut key_preimage = [0u8; 64];
        key_preimage[12..32].copy_from_slice(address.as_slice()); // address at bytes 12-31
        key_preimage[32..64].copy_from_slice(&U256::from(slot).to_be_bytes::<32>()); // slot at bytes 32-63

        let storage_key = keccak256(key_preimage);
        let storage_key = storage_key.into_u256();

        let balance = db.storage_ref(token, storage_key).rpc_err()?;
        Ok(balance)
    }
}

/// Constructs a new database instance for the chain. Stacks the alloy_db, cache_db,
/// and all necesary layer_dbs.
fn construct_db<N: Network>(
    transport: Transport,
    key: String,
    fork_url: String,
    fork_block_number: u64,
    latest_block_number: u64,
) -> Result<Box<dyn DatabaseRef<Error = CacheDBError<AlloyDBError>>>, ChainError> {
    let alloy_db: RemoteDB<N> = RemoteDB::new(
        transport.clone(),
        fork_url,
        BlockId::number(fork_block_number),
    );
    let cache_db_key = format!("{}/{}", key, fork_block_number);
    let cache_db: CacheDB<_, N> = CacheDB::new(transport.clone(), cache_db_key, alloy_db);

    //? Stack layerDBs from 1+forked block to latest block.
    let mut layer_db: LayeredDB<_, N> = LayeredDB::new(cache_db);
    for block_number in (fork_block_number + 1)..=latest_block_number {
        let layer_state_key = get_layer_key(&key, block_number);
        let layer_state: LayerState = transport.state().try_read_key(layer_state_key).rpc_err()?;

        layer_db.push_layer(layer_state);
    }

    Ok(Box::new(layer_db))
}

fn get_blockenv(state: &ChainState, block: &BlockId) -> Option<BlockEnv> {
    let number = block_id_to_number(state, block)?;
    if number == state.pending.env.number.saturating_to::<u64>() {
        return Some(state.pending.env.clone());
    }

    if let Some(env) = state.blocks.get(&number) {
        return Some(env.env.clone());
    }

    return None;
}

fn block_id_to_number(state: &ChainState, block: &BlockId) -> Option<u64> {
    // TODO: Add cases for remaining tags (finalized, safe, earliest) and hashes
    match block {
        BlockId::Number(BlockNumberOrTag::Pending) => {
            Some(state.pending.env.number.saturating_to())
        }
        BlockId::Number(BlockNumberOrTag::Latest) => {
            let latest: u64 = state.pending.env.number.saturating_to();
            Some(latest.saturating_sub(1))
        }
        BlockId::Number(BlockNumberOrTag::Number(n)) => Some(*n),
        _ => None,
    }
}

fn compute_block_hash(number: u64, parent_hash: B256) -> B256 {
    keccak256([number.to_be_bytes().as_slice(), parent_hash.as_slice()].concat())
}

fn apply_state_overrides<DB: DatabaseRef>(
    db: &mut RevmCacheDB<DB>,
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
