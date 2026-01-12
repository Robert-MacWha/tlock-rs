use alloy_consensus::transaction::Recovered;
use revm::{
    DatabaseRef,
    context::result::{ExecutionResult, HaltReason, Output},
    primitives::{
        Address, Bytes, HashMap, U256,
        alloy_primitives::TxHash,
        hex::{self},
    },
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tlock_pdk::{
    state::StateExt,
    tlock_api::alloy::{
        self,
        consensus::{TxEnvelope, transaction::SignerRecoverable},
        eips::{BlockId, BlockNumberOrTag},
        rlp::Decodable,
        rpc::{
            self,
            types::{
                BlockOverrides, BlockTransactions, BlockTransactionsKind, state::StateOverride,
            },
        },
    },
    wasmi_plugin_pdk::{rpc_message::RpcError, transport::Transport},
};
use tracing::info;

use crate::{
    chain::{Chain, ChainError},
    rpc::{
        result_to_tx_receipt, signed_tx_to_tx_env, simulated_block_to_header, tx_request_to_tx_env,
    },
    state::get_provider_key,
};

/// A alloy-style provider backended using REVM. Handles type conversions and
/// chain state management.
///
/// TODO: Consider making this + the chain functional, since they should be
/// stateless
pub struct Provider {
    key: String,
    transport: Transport,
    chain: Chain,
    pub state: ProviderState,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderState {
    pub chain_id: u64,
    pub fork_block: u64,
    pub transactions: HashMap<u64, Vec<rpc::types::Transaction>>,
    pub receipts: HashMap<TxHash, rpc::types::TransactionReceipt>,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Chain Error: {0}")]
    Chain(#[from] ChainError),

    #[error("RLP Error: {0}")]
    RlpError(#[from] alloy::rlp::Error),

    #[error("Recovery Error: {0}")]
    RecoveryError(#[from] alloy::consensus::crypto::RecoveryError),

    #[error("Not Latest Block")]
    NotLatestBlock,

    #[error("Account Not Found")]
    AccountNotFound,

    #[error("Block Not Found")]
    BlockNotFound,

    #[error("Transaction Not Found")]
    TransactionNotFound,

    #[error("Transaction Reverted: {0}")]
    TransactionReverted(String),

    #[error("Chain Halted: {0:?}")]
    ChainHalted(HaltReason),

    #[error("Lock Error: {0}")]
    LockError(#[from] tlock_pdk::state::LockError),

    #[error("Not Implemented")]
    NotImplemented,
}

impl From<ProviderError> for RpcError {
    fn from(err: ProviderError) -> Self {
        RpcError::Custom(err.to_string())
    }
}

impl Provider {
    /// Creates a new Provider instance for a forked chain, initializing state.
    pub fn new(
        transport: Transport,
        key: String,
        fork_url: String,
        header: rpc::types::Header,
        chain_id: u64,
        block_time: u64,
    ) -> Result<Self, ProviderError> {
        let state = ProviderState {
            chain_id,
            fork_block: header.number.saturating_sub(1),
            transactions: HashMap::default(),
            receipts: HashMap::default(),
        };
        let state_key = get_provider_key(&key);
        info!("Writing provider state to key: {}...", state_key);
        transport.state().write_key(state_key, state.clone())?;

        let chain = Chain::new(
            transport.clone(),
            key.clone(),
            fork_url,
            header,
            chain_id,
            block_time,
        )?;

        Ok(Self {
            key,
            transport,
            chain,
            state,
        })
    }

    /// Loads an existing Provider instance from stored state
    pub fn load(
        transport: Transport,
        key: String,
        fork_url: String,
    ) -> Result<Self, ProviderError> {
        let state_key = get_provider_key(&key);
        let state: ProviderState = transport.state().read_key(state_key)?;
        let chain = Chain::load(transport.clone(), key.clone(), fork_url);

        Ok(Self {
            key,
            transport,
            chain,
            state,
        })
    }
}

impl Provider {
    pub fn block_number(&self) -> Result<u64, ProviderError> {
        Ok(self.chain.latest()?)
    }

    pub fn gas_price(&self) -> Result<u128, ProviderError> {
        let pending = self.chain.pending()?;
        Ok(pending.env.basefee as u128)
    }

    pub fn get_balance(&self, address: Address, block_id: BlockId) -> Result<U256, ProviderError> {
        let account = self
            .chain
            .db(block_id)?
            .basic_ref(address)
            .map_err(|e| ChainError::Db(e.to_string()))?
            .unwrap_or_default();

        Ok(account.balance)
    }

    pub fn get_block(
        &self,
        block_id: BlockId,
        tx_kind: BlockTransactionsKind,
    ) -> Result<rpc::types::Block, ProviderError> {
        let number = match block_id {
            BlockId::Number(BlockNumberOrTag::Number(num)) => num,
            BlockId::Number(BlockNumberOrTag::Latest) => self.chain.latest()?,
            _ => return Err(ProviderError::BlockNotFound),
        };

        let simulated_block = self
            .chain
            .block(number)?
            .ok_or(ProviderError::BlockNotFound)?;
        let header = simulated_block_to_header(&simulated_block);
        let block = rpc::types::Block::empty(header);

        let transactions = self
            .state
            .transactions
            .get(&number)
            .cloned()
            .unwrap_or_default();
        let block = match tx_kind {
            BlockTransactionsKind::Hashes => block.with_transactions(BlockTransactions::Hashes(
                transactions
                    .iter()
                    .map(|tx| tx.inner.hash().clone())
                    .collect(),
            )),
            BlockTransactionsKind::Full => {
                block.with_transactions(BlockTransactions::Full(transactions))
            }
        };

        Ok(block)
    }

    pub fn get_code(
        &self,
        address: Address,
        block_id: BlockId,
    ) -> Result<Option<Bytes>, ProviderError> {
        let account = self
            .chain
            .db(block_id)?
            .basic_ref(address)
            .map_err(|e| ChainError::Db(e.to_string()))?
            .unwrap_or_default();

        Ok(account.code.map(|c| c.bytes()))
    }

    pub fn get_transaction_count(
        &self,
        address: Address,
        block_id: BlockId,
    ) -> Result<u64, ProviderError> {
        let account = self
            .chain
            .db(block_id)?
            .basic_ref(address)
            .map_err(|e| ChainError::Db(e.to_string()))?
            .unwrap_or_default();

        Ok(account.nonce)
    }

    pub fn get_transaction_by_hash(
        &self,
        tx_hash: TxHash,
    ) -> Result<rpc::types::Transaction, ProviderError> {
        // TODO: Pass to chain db if not present?
        for txs in self.state.transactions.values() {
            for tx in txs {
                if tx.inner.hash() == &tx_hash {
                    return Ok(tx.clone());
                }
            }
        }

        Err(ProviderError::TransactionNotFound)
    }

    pub fn call(
        &self,
        tx_request: rpc::types::TransactionRequest,
        block_id: BlockId,
        state_override: Option<StateOverride>,
        block_override: Option<BlockOverrides>,
    ) -> Result<Bytes, ProviderError> {
        let tx_env = tx_request_to_tx_env(tx_request);

        let result = self
            .chain
            .call(tx_env, block_id, state_override, block_override, true)?;

        match result {
            ExecutionResult::Success { output, .. } => match output {
                Output::Call(bytes) => Ok(bytes),
                Output::Create(bytes, _) => Ok(bytes),
            },
            ExecutionResult::Revert { output, .. } => Err(ProviderError::TransactionReverted(
                decode_revert_reason(&output),
            )),
            ExecutionResult::Halt { reason, .. } => Err(ProviderError::ChainHalted(reason)),
        }
    }

    pub fn estimate_gas(
        &self,
        tx_request: rpc::types::TransactionRequest,
        block_id: BlockId,
        state_override: Option<StateOverride>,
        block_override: Option<BlockOverrides>,
    ) -> Result<u64, ProviderError> {
        let tx_env = tx_request_to_tx_env(tx_request);

        let result = self
            .chain
            .call(tx_env, block_id, state_override, block_override, true)?;

        match result {
            ExecutionResult::Success { gas_used, .. } => Ok((gas_used * 120) / 100), //? Add 20% buffer
            ExecutionResult::Revert { output, .. } => Err(ProviderError::TransactionReverted(
                decode_revert_reason(&output),
            )),
            ExecutionResult::Halt { reason, .. } => Err(ProviderError::ChainHalted(reason)),
        }
    }

    /// Sends a raw transaction to the chain, executes it, and returns its hash.
    pub fn send_raw_transaction(&self, raw_tx: Bytes) -> Result<TxHash, ProviderError> {
        let state_key = get_provider_key(&self.key);
        let mut state = self
            .transport
            .state()
            .try_lock_key::<ProviderState>(state_key)?;

        let tx_envelope = TxEnvelope::decode(&mut raw_tx.as_ref())?;
        let from = tx_envelope.recover_signer()?;
        let tx_hash = tx_envelope.hash().clone();

        let tx_env = signed_tx_to_tx_env(&tx_envelope, from);
        let result = self.chain.transact_commit(tx_env)?;

        //? Should always have a block since we just committed a transaction
        let block_number = self.chain.latest()?;
        let block = self
            .chain
            .block(block_number)?
            .ok_or(ProviderError::BlockNotFound)?;

        let receipt = result_to_tx_receipt(&block, tx_envelope.clone(), from, &result);
        state.receipts.insert(tx_hash, receipt);

        let rpc_tx = rpc::types::Transaction {
            inner: Recovered::new_unchecked(tx_envelope, from),
            block_hash: Some(block.hash),
            block_number: Some(block_number),
            transaction_index: Some(
                state
                    .transactions
                    .get(&block_number)
                    .map_or(0, |v| v.len() as u64),
            ),
            effective_gas_price: Some(self.chain.pending()?.env.basefee as u128),
        };
        state
            .transactions
            .entry(block_number)
            .or_insert_with(Vec::new)
            .push(rpc_tx);

        Ok(tx_hash)
    }

    pub fn get_transaction_receipt(
        &self,
        tx_hash: TxHash,
    ) -> Option<rpc::types::TransactionReceipt> {
        self.state.receipts.get(&tx_hash).cloned()
    }

    pub fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<rpc::types::TransactionReceipt>, ProviderError> {
        let number = match block_id {
            BlockId::Number(BlockNumberOrTag::Number(num)) => num,
            BlockId::Number(BlockNumberOrTag::Latest) => self.chain.latest()?,
            _ => return Err(ProviderError::BlockNotFound),
        };

        let Some(txns) = self.state.transactions.get(&number) else {
            return Ok(vec![]);
        };

        let receipts = txns
            .iter()
            .filter_map(|tx| self.state.receipts.get(&tx.inner.hash().clone()).cloned())
            .collect();

        Ok(receipts)
    }

    pub fn get_logs(
        &self,
        _filter: rpc::types::Filter,
    ) -> Result<Vec<rpc::types::Log>, ProviderError> {
        // TODO: Impl get_logs
        return Err(ProviderError::NotImplemented);
    }

    pub fn fee_history(
        &self,
        block_count: u64,
        newest_block: BlockNumberOrTag,
        reward_percentiles: Vec<f64>,
    ) -> Result<alloy::rpc::types::FeeHistory, ProviderError> {
        let count: usize = block_count as usize;
        let pending = self.chain.pending()?;

        let current_base_fee: u128 = pending.env.basefee.into();
        let blob_excess_gas_and_price: u128 = pending
            .env
            .blob_excess_gas_and_price
            .map(|b| b.blob_gasprice)
            .unwrap_or(1u128);

        let newest_num: u64 = match newest_block {
            BlockNumberOrTag::Number(n) => n,
            BlockNumberOrTag::Latest => self.chain.latest()?,
            BlockNumberOrTag::Pending => self.chain.latest()?,
            _ => return Err(ProviderError::BlockNotFound),
        };
        let oldest_block = newest_num.saturating_sub(block_count);

        Ok(alloy::rpc::types::FeeHistory {
            oldest_block,
            base_fee_per_gas: vec![current_base_fee; count + 1],
            gas_used_ratio: vec![0.1; count as usize],
            reward: Some(vec![
                vec![1_000_000_000u128; reward_percentiles.len()];
                count
            ]),
            base_fee_per_blob_gas: vec![blob_excess_gas_and_price; count + 1],
            blob_gas_used_ratio: vec![0.1; count as usize],
        })
    }
}

// ---------- CHEATCODES ----------
#[allow(dead_code)]
impl Provider {
    /// Sets the balance for a given address.
    pub fn deal(&self, address: Address, amount: U256) -> Result<(), ProviderError> {
        Ok(self.chain.deal(address, amount)?)
    }

    /// Sets the ERC20 token balance for a given address.
    pub fn deal_erc20(
        &self,
        address: Address,
        token: Address,
        amount: U256,
    ) -> Result<(), ProviderError> {
        Ok(self.chain.deal_erc20(address, token, amount)?)
    }

    /// Mines a new block.
    pub fn mine(&self) -> Result<(), ProviderError> {
        self.chain.mine()?;
        Ok(())
    }
}

fn decode_revert_reason(bytes: &[u8]) -> String {
    // 0x08c379a0 is the selector for Error(string)
    if bytes.len() < 4 || bytes[0..4] != [0x08, 0xc3, 0x79, 0xa0] {
        return format!("Raw Revert: 0x{}", hex::encode(bytes));
    }

    // Standard ABI encoding for strings:
    // [0:4] Selector
    // [4:36] Offset to string data (usually 0x20)
    // [36:68] Length of string
    // [68..] String data
    if bytes.len() < 68 {
        return format!("Malformed Revert: 0x{}", hex::encode(bytes));
    }

    let length = u32::from_be_bytes(bytes[64..68].try_into().unwrap_or([0; 4])) as usize;
    let data_end = 68 + length;

    if bytes.len() < data_end {
        return format!("Truncated Revert: 0x{}", hex::encode(bytes));
    }

    match String::from_utf8(bytes[68..data_end].to_vec()) {
        Ok(s) => s,
        Err(_) => format!("Hex Revert: 0x{}", hex::encode(bytes)),
    }
}
