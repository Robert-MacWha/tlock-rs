use revm::{
    Database, DatabaseRef,
    context::{
        BlockEnv,
        result::{ExecutionResult, HaltReason, Output},
    },
    primitives::{
        Address, Bytes, HashMap, U256,
        alloy_primitives::{BlockHash, TxHash},
        hex::ToHexExt,
        keccak256,
    },
};
use thiserror::Error;
use tlock_pdk::{
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
        sol_types::SolValue,
    },
    wasmi_plugin_pdk::rpc_message::RpcError,
};

use crate::{
    chain::{Chain, ChainError},
    provider_snapshot::ProviderSnapshot,
    rpc::{
        result_to_tx_receipt, signed_tx_to_tx_env, simulated_block_to_header, tx_request_to_tx_env,
    },
};

/// A alloy-style provider backended using REVM. Handles type conversions and
/// chain state management.
pub struct Provider<DB: DatabaseRef> {
    chain: Chain<DB>,

    /// Cache of all transactions sent to this provider, organized by block
    /// number.
    transactions: HashMap<u64, Vec<rpc::types::Transaction>>,
    receipts: HashMap<TxHash, rpc::types::TransactionReceipt>,
}

#[derive(Debug, Error)]
pub enum ProviderError<DB: DatabaseRef> {
    #[error("Chain Error: {0}")]
    Chain(#[from] ChainError<DB>),

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

    #[error("Not Implemented")]
    NotImplemented,
}

impl<DB: DatabaseRef> From<ProviderError<DB>> for RpcError {
    fn from(err: ProviderError<DB>) -> Self {
        RpcError::Custom(err.to_string())
    }
}

impl<DB: DatabaseRef + std::fmt::Debug> Provider<DB> {
    pub fn new(db: DB, block_env: BlockEnv, parent_hash: BlockHash) -> Self {
        let chain = Chain::new(db, block_env, Some(parent_hash));
        Self {
            chain,
            transactions: HashMap::default(),
            receipts: HashMap::default(),
        }
    }

    pub fn from_snapshot(db: DB, snapshot: ProviderSnapshot) -> Self {
        let chain = Chain::from_snapshot(db, snapshot.chain);
        Self {
            chain,
            transactions: snapshot.transactions,
            receipts: snapshot.receipts,
        }
    }

    pub fn snapshot(&self) -> ProviderSnapshot {
        ProviderSnapshot {
            chain: self.chain.snapshot(),
            transactions: self.transactions.clone(),
            receipts: self.receipts.clone(),
        }
    }
}

impl<DB: DatabaseRef> Provider<DB> {
    pub fn block_number(&self) -> u64 {
        self.chain.latest()
    }

    pub fn gas_price(&self) -> u128 {
        self.chain.pending().env.basefee as u128
    }

    pub fn get_balance(
        &mut self,
        address: Address,
        block_id: BlockId,
    ) -> Result<U256, ProviderError<DB>> {
        if !self.is_latest(block_id) {
            return Err(ProviderError::NotLatestBlock);
        }

        let account = self
            .chain
            .db()
            .basic(address)
            .map_err(|e| ChainError::Db(e.to_string()))?
            .ok_or(ProviderError::AccountNotFound)?;

        Ok(account.balance)
    }

    pub fn get_block(
        &self,
        block_id: BlockId,
        tx_kind: BlockTransactionsKind,
    ) -> Result<rpc::types::Block, ProviderError<DB>> {
        let number = match block_id {
            BlockId::Number(BlockNumberOrTag::Number(num)) => num,
            BlockId::Number(BlockNumberOrTag::Latest) => self.chain.latest(),
            _ => return Err(ProviderError::BlockNotFound),
        };

        let simulated_block = self
            .chain
            .block(number)
            .ok_or(ProviderError::BlockNotFound)?;
        let header = simulated_block_to_header(simulated_block);
        let block = rpc::types::Block::empty(header);

        let transactions = self.transactions.get(&number).cloned().unwrap_or_default();
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
        &mut self,
        address: Address,
        block_id: BlockId,
    ) -> Result<Option<Bytes>, ProviderError<DB>> {
        if !self.is_latest(block_id) {
            return Err(ProviderError::NotLatestBlock);
        }

        let account = self
            .chain
            .db()
            .basic(address)
            .map_err(|e| ChainError::Db(e.to_string()))?
            .ok_or(ProviderError::AccountNotFound)?;

        Ok(account.code.map(|c| c.bytes()))
    }

    pub fn get_transaction_count(
        &mut self,
        address: Address,
        block_id: BlockId,
    ) -> Result<u64, ProviderError<DB>> {
        if !self.is_latest(block_id) {
            return Err(ProviderError::NotLatestBlock);
        }

        let account = self
            .chain
            .db()
            .basic(address)
            .map_err(|e| ChainError::Db(e.to_string()))?
            .ok_or(ProviderError::AccountNotFound)?;

        Ok(account.nonce)
    }

    pub fn get_transaction_by_hash(
        &self,
        tx_hash: TxHash,
    ) -> Result<rpc::types::Transaction, ProviderError<DB>> {
        for txs in self.transactions.values() {
            for tx in txs {
                if tx.inner.hash() == &tx_hash {
                    return Ok(tx.clone());
                }
            }
        }

        Err(ProviderError::BlockNotFound)
    }

    pub fn call(
        &self,
        tx_request: rpc::types::TransactionRequest,
        block_id: BlockId,
        state_override: Option<StateOverride>,
        block_override: Option<BlockOverrides>,
    ) -> Result<Bytes, ProviderError<DB>> {
        let tx_env = tx_request_to_tx_env(tx_request);

        let result = self
            .chain
            .call(tx_env, block_id, state_override, block_override)?;

        match result {
            ExecutionResult::Success { output, .. } => match output {
                Output::Call(bytes) => Ok(bytes),
                Output::Create(bytes, _) => Ok(bytes),
            },
            ExecutionResult::Revert { output, .. } => Err(
                ProviderError::<DB>::TransactionReverted(output.encode_hex()),
            ),
            ExecutionResult::Halt { reason, .. } => Err(ProviderError::<DB>::ChainHalted(reason)),
        }
    }

    pub fn estimate_gas(
        &self,
        tx_request: rpc::types::TransactionRequest,
        block_id: BlockId,
        state_override: Option<StateOverride>,
        block_override: Option<BlockOverrides>,
    ) -> Result<u64, ProviderError<DB>> {
        let tx_env = tx_request_to_tx_env(tx_request);

        let result = self
            .chain
            .call(tx_env, block_id, state_override, block_override)?;

        match result {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { output, .. } => Err(
                ProviderError::<DB>::TransactionReverted(output.encode_hex()),
            ),
            ExecutionResult::Halt { reason, .. } => Err(ProviderError::<DB>::ChainHalted(reason)),
        }
    }

    /// Sends a raw transaction to the chain, executes it, and returns its hash.
    pub fn send_raw_transaction(&mut self, raw_tx: Bytes) -> Result<TxHash, ProviderError<DB>> {
        let tx_envelope = TxEnvelope::decode(&mut raw_tx.as_ref())?;
        let from = tx_envelope.recover_signer()?;

        let tx_env = signed_tx_to_tx_env(&tx_envelope, from);
        let result = self.chain.transact_commit(tx_env)?;

        //? Should always have a block since we just committed a transaction
        let block = self
            .chain
            .block(self.chain.latest())
            .ok_or(ProviderError::BlockNotFound)?;

        let receipt = result_to_tx_receipt(block, tx_envelope, from, &result);
        let tx_hash = receipt.transaction_hash.clone();
        self.receipts.insert(tx_hash, receipt);

        Ok(tx_hash)
    }

    pub fn get_transaction_receipt(
        &self,
        tx_hash: TxHash,
    ) -> Option<rpc::types::TransactionReceipt> {
        self.receipts.get(&tx_hash).cloned()
    }

    pub fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<rpc::types::TransactionReceipt>, ProviderError<DB>> {
        let number = match block_id {
            BlockId::Number(BlockNumberOrTag::Number(num)) => num,
            BlockId::Number(BlockNumberOrTag::Latest) => self.chain.latest(),
            _ => return Err(ProviderError::BlockNotFound),
        };

        let Some(txns) = self.transactions.get(&number) else {
            return Ok(vec![]);
        };

        let receipts = txns
            .iter()
            .filter_map(|tx| self.receipts.get(&tx.inner.hash().clone()).cloned())
            .collect();

        Ok(receipts)
    }

    pub fn get_logs(
        &self,
        _filter: rpc::types::Filter,
    ) -> Result<Vec<rpc::types::Log>, ProviderError<DB>> {
        // TODO: Impl get_logs
        return Err(ProviderError::NotImplemented);
    }
}

// ---------- CHEATCODES ----------
#[allow(dead_code)]
impl<DB: DatabaseRef> Provider<DB> {
    /// Sets the balance for a given address.
    pub fn deal(&mut self, address: Address, amount: U256) -> Result<(), ProviderError<DB>> {
        let mut info = self
            .chain
            .db()
            .basic(address)
            .map_err(|e| ChainError::Db(e.to_string()))?
            .unwrap_or_default();

        info.balance = amount;
        self.chain.db().insert_account_info(address, info);

        Ok(())
    }

    /// Sets the ERC20 token balance for a given address by directly modifying
    /// the storage slot corresponding to the balance mapping.
    ///
    /// Uses the standard Solidity storage layout for mappings, where the slot
    /// is calculated as keccak256(abi.encode(address, slot_number)). Will
    /// not work for non-standard address mappings, viper contracts, etc.
    pub fn deal_erc20(
        &mut self,
        address: Address,
        erc20: Address,
        amount: U256,
    ) -> Result<(), ProviderError<DB>> {
        let slot = self.erc_address_storage(address);
        let mut storage = HashMap::default();
        storage.insert(slot, amount);

        self.chain
            .db()
            .replace_account_storage(erc20, storage)
            .map_err(|e| ChainError::Db(e.to_string()))?;

        Ok(())
    }

    /// Mines a new block.
    pub fn mine(&mut self) -> Result<(), ProviderError<DB>> {
        self.chain.mine()?;
        Ok(())
    }

    /// Calculates the storage slot for an ERC20 balance mapping.
    /// This implements the standard Solidity mapping storage layout where
    /// slot = keccak256(abi.encode(address, slot_number))
    fn erc_address_storage(&self, address: Address) -> U256 {
        keccak256((address, U256::from(4)).abi_encode()).into()
    }
}

impl<DB: DatabaseRef> Provider<DB> {
    fn is_latest(&self, block: BlockId) -> bool {
        match block {
            BlockId::Number(BlockNumberOrTag::Latest) => true,
            BlockId::Number(num) => num == BlockNumberOrTag::Number(self.chain.latest()),
            _ => false,
        }
    }
}
