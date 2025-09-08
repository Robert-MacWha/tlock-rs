use alloy_dyn_abi::TypedData;
use alloy_primitives::{
    Address, BlockHash, BlockNumber, Bytes, ChainId, Signature, TxHash, U64, U128, U256,
};
use alloy_rpc_types::{
    Block, BlockNumberOrTag, EIP1186AccountProofResponse, FeeHistory, Filter, FilterChanges, Log,
    SyncStatus, Transaction, TransactionReceipt, TransactionRequest, pubsub::SubscriptionKind,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{api::ApiError, rpc_message::RpcErrorCode};

#[derive(Serialize, Deserialize)]
pub struct EthCallParams {
    pub transaction: TransactionRequest,
    pub block: BlockNumberOrTag,
}

#[derive(Serialize, Deserialize)]
pub struct EthEstimateGasParams {
    pub transaction: TransactionRequest,
    pub block: BlockNumberOrTag,
}

#[derive(Serialize, Deserialize)]
pub struct EthFeeHistoryParams {
    pub block_count: BlockNumber,
    pub last_block: BlockNumberOrTag,
    pub reward_percentiles: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct EthGetBalanceParams {
    pub address: Address,
    pub block: BlockNumberOrTag,
}

#[derive(Serialize, Deserialize)]
pub struct EthGetBlockByHashParams {
    pub hash: BlockHash,
    pub hydrate_transactions: bool,
}

#[derive(Serialize, Deserialize)]
pub struct EthGetBlockByNumberParams {
    pub block: BlockNumberOrTag,
    pub hydrate_transactions: bool,
}

#[derive(Serialize, Deserialize)]
pub struct EthGetCodeParams {
    pub address: Address,
    pub block: BlockNumberOrTag,
}

#[derive(Serialize, Deserialize)]
pub struct EthGetProofParams {
    pub address: Address,
    pub key: U256,
}

#[derive(Serialize, Deserialize)]
pub struct EthGetStorageAtParams {
    pub address: Address,
    pub key: U256,
    pub block: BlockNumberOrTag,
}

#[derive(Serialize, Deserialize)]
pub struct EthGetTransactionByBlockHashAndIndexParams {
    pub block_hash: BlockHash,
    pub index: usize,
}

#[derive(Serialize, Deserialize)]
pub struct EthSubscribeParams {
    pub subscription_type: SubscriptionKind,
    pub filter: Option<Filter>,
}

#[derive(Serialize, Deserialize)]
pub struct EthDecryptParams {
    pub encrypted: Bytes,
    pub address: Address,
}

#[derive(Serialize, Deserialize)]
pub struct EthSignTypedDataV4Params {
    pub address: Address,
    pub data: TypedData,
}

#[derive(Serialize, Deserialize)]
pub struct PersonalSignParams {
    pub data: Bytes,
    pub address: Address,
}

/// https://ethereum.org/en/developers/docs/apis/json-rpc/#json-rpc-methods
#[async_trait]
#[allow(unused_variables)]
pub trait EthNamespace<E: ApiError>: Send + Sync {
    async fn eth_accounts(&self) -> Result<Vec<Address>, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_block_number(&self) -> Result<BlockNumber, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_call(&self, params: EthCallParams) -> Result<Bytes, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_chain_id(&self) -> Result<ChainId, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_coinbase(&self) -> Result<Address, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_decrypt(&self, encrypted: Bytes, address: Address) -> Result<String, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_estimate_gas(
        &self,
        tx: TransactionRequest,
        block: BlockNumberOrTag,
    ) -> Result<u64, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_fee_history(
        &self,
        block_count: BlockNumber,
        last_block: BlockNumberOrTag,
        reward_percentiles: Vec<f64>,
    ) -> Result<FeeHistory, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_gas_price(&self) -> Result<U128, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_balance(&self, address: Address, block: BlockNumberOrTag) -> Result<U256, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_block_by_hash(
        &self,
        hash: BlockHash,
        hydrate_transactions: bool,
    ) -> Result<Block, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        hydrate_transactions: bool,
    ) -> Result<Block, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_block_transaction_count_by_hash(&self, hash: BlockHash) -> Result<U64, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_block_transaction_count_by_number(
        &self,
        block: BlockNumberOrTag,
    ) -> Result<U64, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_code(&self, address: Address, block: BlockNumberOrTag) -> Result<Bytes, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_encryption_public_key(&self, address: Address) -> Result<Bytes, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_logs(&self, filter: Filter) -> Result<Vec<Log>, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_proof(
        &self,
        address: Address,
        key: U256,
    ) -> Result<EIP1186AccountProofResponse, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: BlockNumberOrTag,
    ) -> Result<U256, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_transaction_by_block_hash_and_index(
        &self,
        block_hash: BlockHash,
        index: usize,
    ) -> Result<Transaction, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_transaction_by_hash(&self, hash: TxHash) -> Result<Transaction, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_transaction_receipt(&self, hash: TxHash) -> Result<TransactionReceipt, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_uncle_count_by_block_hash(&self, hash: BlockHash) -> Result<U64, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_get_uncle_count_by_block_number(&self, block: BlockNumberOrTag) -> Result<U64, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_new_block_filter(&self) -> Result<U256, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_new_filter(&self, _filter: Filter) -> Result<U256, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_new_pending_transaction_filter(&self) -> Result<U256, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_send_raw_transaction(&self, tx: Bytes) -> Result<TxHash, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_send_transaction(&self, tx: TransactionRequest) -> Result<TxHash, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_sign_typed_data_v4(
        &self,
        address: Address,
        data: TypedData,
    ) -> Result<Signature, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_syncing(&self) -> Result<SyncStatus, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_subscribe(
        &self,
        kind: SubscriptionKind,
        filter: Option<Filter>,
    ) -> Result<String, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_unsubscribe(&self, subscription_id: String) -> Result<bool, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn eth_uninstall_filter(&self, filter_id: U256) -> Result<bool, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }

    async fn personal_sign(&self, data: Bytes, address: Address) -> Result<Signature, E> {
        Err(RpcErrorCode::MethodNotSupported.into())
    }
    // TODO: This is a Metamask method, not officially part of eth json-rpc. Should it be included?
    // async fn eth_request_accounts(&self) -> Result<Vec<Address>, E>;
}
