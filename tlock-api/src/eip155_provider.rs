use alloy_primitives::{Address, BlockHash, BlockNumber, Bytes, ChainId, TxHash, U64, U128, U256};
use alloy_rpc_types::{
    Block, BlockNumberOrTag, EIP1186AccountProofResponse, FeeHistory, Filter, FilterChanges, Log,
    SyncStatus, Transaction, TransactionReceipt, TransactionRequest, pubsub::SubscriptionKind,
};
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use wasmi_pdk::api::ApiError;

use crate::methods::Methods;

/// https://ethereum.org/en/developers/docs/apis/json-rpc/#json-rpc-methods
#[rpc_namespace]
#[async_trait]
pub trait Eip155Provider: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::EthBlockNumber)]
    async fn eth_block_number(&self) -> Result<BlockNumber, Self::Error>;

    #[rpc_method(Methods::EthCall)]
    async fn eth_call(
        &self,
        transaction: TransactionRequest,
        block: BlockNumberOrTag,
    ) -> Result<Bytes, Self::Error>;

    #[rpc_method(Methods::EthChainId)]
    async fn eth_chain_id(&self) -> Result<ChainId, Self::Error>;

    #[rpc_method(Methods::EthCoinbase)]
    async fn eth_coinbase(&self) -> Result<Address, Self::Error>;

    #[rpc_method(Methods::EthSendRawTransaction)]
    /// Sends a raw transaction signed transaction.
    ///  
    ///  https://ethereum.org/developers/docs/apis/json-rpc/#eth_sendrawtransaction
    async fn eth_send_raw_transaction(&self, tx: Bytes) -> Result<TxHash, Self::Error>;

    #[rpc_method(Methods::EthEstimateGas)]
    async fn eth_estimate_gas(
        &self,
        transaction: TransactionRequest,
        block: BlockNumberOrTag,
    ) -> Result<u64, Self::Error>;

    #[rpc_method(Methods::EthFeeHistory)]
    async fn eth_fee_history(
        &self,
        block_count: u64,
        last_block: BlockNumberOrTag,
        reward_percentiles: Vec<f64>,
    ) -> Result<FeeHistory, Self::Error>;

    #[rpc_method(Methods::EthGasPrice)]
    async fn eth_gas_price(&self) -> Result<U128, Self::Error>;

    #[rpc_method(Methods::EthGetBlockByHash)]
    async fn eth_get_block_by_hash(
        &self,
        hash: BlockHash,
        hydrate_transactions: bool,
    ) -> Result<Block, Self::Error>;

    #[rpc_method(Methods::EthGetBlockByNumber)]
    async fn eth_get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        hydrate_transactions: bool,
    ) -> Result<Block, Self::Error>;

    #[rpc_method(Methods::EthGetBlockTransactionCountByHash)]
    async fn eth_get_block_transaction_count_by_hash(
        &self,
        hash: BlockHash,
    ) -> Result<U64, Self::Error>;

    #[rpc_method(Methods::EthGetBlockTransactionCountByNumber)]
    async fn eth_get_block_transaction_count_by_number(
        &self,
        block: BlockNumberOrTag,
    ) -> Result<U64, Self::Error>;

    #[rpc_method(Methods::EthGetCode)]
    async fn eth_get_code(
        &self,
        address: Address,
        block: BlockNumberOrTag,
    ) -> Result<Bytes, Self::Error>;

    #[rpc_method(Methods::EthGetFilterChanges)]
    async fn eth_get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges, Self::Error>;

    #[rpc_method(Methods::EthGetFilterLogs)]
    async fn eth_get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>, Self::Error>;

    #[rpc_method(Methods::EthGetLogs)]
    async fn eth_get_logs(&self, filter: Filter) -> Result<Vec<Log>, Self::Error>;

    #[rpc_method(Methods::EthGetProof)]
    async fn eth_get_proof(
        &self,
        address: Address,
        key: U256,
    ) -> Result<EIP1186AccountProofResponse, Self::Error>;

    #[rpc_method(Methods::EthGetStorageAt)]
    async fn eth_get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: BlockNumberOrTag,
    ) -> Result<U256, Self::Error>;

    #[rpc_method(Methods::EthGetTransactionByBlockHashAndIndex)]
    async fn eth_get_transaction_by_block_hash_and_index(
        &self,
        block_hash: BlockHash,
        index: usize,
    ) -> Result<Transaction, Self::Error>;

    #[rpc_method(Methods::EthGetTransactionByHash)]
    async fn eth_get_transaction_by_hash(&self, hash: TxHash) -> Result<Transaction, Self::Error>;

    #[rpc_method(Methods::EthGetTransactionReceipt)]
    async fn eth_get_transaction_receipt(
        &self,
        hash: TxHash,
    ) -> Result<TransactionReceipt, Self::Error>;

    #[rpc_method(Methods::EthGetTransactionCount)]
    async fn eth_get_uncle_count_by_block_hash(&self, hash: BlockHash) -> Result<U64, Self::Error>;

    #[rpc_method(Methods::EthGetUncleCountByBlockNumber)]
    async fn eth_get_uncle_count_by_block_number(
        &self,
        block: BlockNumberOrTag,
    ) -> Result<U64, Self::Error>;

    #[rpc_method(Methods::EthNewBlockFilter)]
    async fn eth_new_block_filter(&self) -> Result<U256, Self::Error>;

    #[rpc_method(Methods::EthNewFilter)]
    async fn eth_new_filter(&self, _filter: Filter) -> Result<U256, Self::Error>;

    #[rpc_method(Methods::EthNewPendingTransactionFilter)]
    async fn eth_new_pending_transaction_filter(&self) -> Result<U256, Self::Error>;

    #[rpc_method(Methods::EthSyncing)]
    async fn eth_syncing(&self) -> Result<SyncStatus, Self::Error>;

    #[rpc_method(Methods::EthSubscribe)]
    async fn eth_subscribe(
        &self,
        subscription_type: SubscriptionKind,
        filter: Option<Filter>,
    ) -> Result<String, Self::Error>;

    #[rpc_method(Methods::EthUnsubscribe)]
    async fn eth_unsubscribe(&self, subscription_id: String) -> Result<bool, Self::Error>;

    #[rpc_method(Methods::EthUninstallFilter)]
    async fn eth_uninstall_filter(&self, filter_id: U256) -> Result<bool, Self::Error>;

    // TODO: This is a Metamask method, not officially part of eth json-rpc. Should it be included?
    // async fn eth_request_accounts(&self) -> Result<Vec<Address>, Self::Error>;
}
