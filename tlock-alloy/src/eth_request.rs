use crate::serde_helpers::{
    common::deserialize_number, empty_params, lenient_block_number, sequence,
};
/// https://github.com/foundry-rs/foundry/blob/a27da27d61dfedfed9c975cac001a48b0f398a55/crates/anvil/core/src/eth/mod.rs
/// Licensed under Apache-2.0 OR MIT.  Copyright (c) 2021 Georgios Konstantopoulos
/// Copied and adapter for tlock-rs.
///
/// Modifications were required to allow compilation to wasm32-unknown-unknown target. This is mostly
/// a direct copy, removing dependencies (which were irrelevant to the deserialization function but
/// present in the original file). I also updated some types to modern import paths from alloy so
/// I could use the same version as elsewhere.
use alloy::{
    dyn_abi::TypedData,
    eips::{BlockId, BlockNumberOrTag},
    primitives::{Address, B64, B256, Bytes, TxHash, U256},
    rpc::types::{
        BlockOverrides, Filter, Index, TransactionRequest,
        simulate::SimulatePayload,
        state::StateOverride,
        trace::{filter::TraceFilter, geth::GethDebugTracingCallOptions},
    },
    serde::WithOtherFields,
};

/// Wrapper type that ensures the type is named `params`
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[allow(dead_code)]
pub struct Params<T: Default> {
    #[serde(default)]
    pub params: T,
}

/// Represents ethereum JSON-RPC API
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "method", content = "params")]
#[allow(dead_code)]
#[allow(clippy::large_enum_variant)]
pub enum EthRequest {
    #[serde(rename = "web3_clientVersion", with = "empty_params")]
    Web3ClientVersion(()),

    #[serde(rename = "web3_sha3", with = "sequence")]
    Web3Sha3(Bytes),

    /// Returns the current Ethereum protocol version.
    #[serde(rename = "eth_protocolVersion", with = "empty_params")]
    EthProtocolVersion(()),

    #[serde(rename = "eth_chainId", with = "empty_params")]
    EthChainId(()),

    #[serde(rename = "eth_networkId", alias = "net_version", with = "empty_params")]
    EthNetworkId(()),

    #[serde(rename = "net_listening", with = "empty_params")]
    NetListening(()),

    /// Returns the number of hashes per second with which the node is mining.
    #[serde(rename = "eth_hashrate", with = "empty_params")]
    EthHashrate(()),

    #[serde(rename = "eth_gasPrice", with = "empty_params")]
    EthGasPrice(()),

    #[serde(rename = "eth_maxPriorityFeePerGas", with = "empty_params")]
    EthMaxPriorityFeePerGas(()),

    #[serde(rename = "eth_blobBaseFee", with = "empty_params")]
    EthBlobBaseFee(()),

    #[serde(
        rename = "eth_accounts",
        alias = "eth_requestAccounts",
        with = "empty_params"
    )]
    EthAccounts(()),

    #[serde(rename = "eth_blockNumber", with = "empty_params")]
    EthBlockNumber(()),

    /// Returns the client coinbase address.
    #[serde(rename = "eth_coinbase", with = "empty_params")]
    EthCoinbase(()),

    #[serde(rename = "eth_getBalance")]
    EthGetBalance(Address, Option<BlockId>),

    #[serde(rename = "eth_getAccount")]
    EthGetAccount(Address, Option<BlockId>),

    #[serde(rename = "eth_getAccountInfo")]
    EthGetAccountInfo(Address, Option<BlockId>),

    #[serde(rename = "eth_getStorageAt")]
    EthGetStorageAt(Address, U256, Option<BlockId>),

    #[serde(rename = "eth_getBlockByHash")]
    EthGetBlockByHash(B256, bool),

    #[serde(rename = "eth_getBlockByNumber")]
    EthGetBlockByNumber(
        #[serde(deserialize_with = "lenient_block_number::lenient_block_number")] BlockNumberOrTag,
        bool,
    ),

    #[serde(rename = "eth_getTransactionCount")]
    EthGetTransactionCount(Address, Option<BlockId>),

    #[serde(rename = "eth_getBlockTransactionCountByHash", with = "sequence")]
    EthGetTransactionCountByHash(B256),

    #[serde(
        rename = "eth_getBlockTransactionCountByNumber",
        deserialize_with = "lenient_block_number::lenient_block_number_seq"
    )]
    EthGetTransactionCountByNumber(BlockNumberOrTag),

    #[serde(rename = "eth_getUncleCountByBlockHash", with = "sequence")]
    EthGetUnclesCountByHash(B256),

    #[serde(
        rename = "eth_getUncleCountByBlockNumber",
        deserialize_with = "lenient_block_number::lenient_block_number_seq"
    )]
    EthGetUnclesCountByNumber(BlockNumberOrTag),

    #[serde(rename = "eth_getCode")]
    EthGetCodeAt(Address, Option<BlockId>),

    /// Returns the account and storage values of the specified account including the Merkle-proof.
    /// This call can be used to verify that the data you are pulling from is not tampered with.
    #[serde(rename = "eth_getProof")]
    EthGetProof(Address, Vec<B256>, Option<BlockId>),

    /// The sign method calculates an Ethereum specific signature with:
    #[serde(rename = "eth_sign")]
    EthSign(Address, Bytes),

    /// The sign method calculates an Ethereum specific signature, equivalent to eth_sign:
    /// <https://docs.metamask.io/wallet/reference/personal_sign/>
    #[serde(rename = "personal_sign")]
    PersonalSign(Bytes, Address),

    #[serde(rename = "eth_signTransaction", with = "sequence")]
    EthSignTransaction(Box<WithOtherFields<TransactionRequest>>),

    /// Signs data via [EIP-712](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-712.md).
    #[serde(rename = "eth_signTypedData")]
    EthSignTypedData(Address, serde_json::Value),

    /// Signs data via [EIP-712](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-712.md).
    #[serde(rename = "eth_signTypedData_v3")]
    EthSignTypedDataV3(Address, serde_json::Value),

    /// Signs data via [EIP-712](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-712.md), and includes full support of arrays and recursive data structures.
    #[serde(rename = "eth_signTypedData_v4")]
    EthSignTypedDataV4(Address, TypedData),

    #[serde(rename = "eth_sendTransaction", with = "sequence")]
    EthSendTransaction(Box<WithOtherFields<TransactionRequest>>),

    #[serde(rename = "eth_sendTransactionSync", with = "sequence")]
    EthSendTransactionSync(Box<WithOtherFields<TransactionRequest>>),

    #[serde(rename = "eth_sendRawTransaction", with = "sequence")]
    EthSendRawTransaction(Bytes),

    #[serde(rename = "eth_sendRawTransactionSync", with = "sequence")]
    EthSendRawTransactionSync(Bytes),

    #[serde(rename = "eth_call")]
    EthCall(
        WithOtherFields<TransactionRequest>,
        #[serde(default)] Option<BlockId>,
        #[serde(default)] Option<StateOverride>,
        #[serde(default)] Option<Box<BlockOverrides>>,
    ),

    #[serde(rename = "eth_simulateV1")]
    EthSimulateV1(SimulatePayload, #[serde(default)] Option<BlockId>),

    #[serde(rename = "eth_createAccessList")]
    EthCreateAccessList(
        WithOtherFields<TransactionRequest>,
        #[serde(default)] Option<BlockId>,
    ),

    #[serde(rename = "eth_estimateGas")]
    EthEstimateGas(
        WithOtherFields<TransactionRequest>,
        #[serde(default)] Option<BlockId>,
        #[serde(default)] Option<StateOverride>,
        #[serde(default)] Option<Box<BlockOverrides>>,
    ),

    #[serde(rename = "eth_fillTransaction", with = "sequence")]
    EthFillTransaction(WithOtherFields<TransactionRequest>),

    #[serde(rename = "eth_getTransactionByHash", with = "sequence")]
    EthGetTransactionByHash(TxHash),

    /// Returns the blob for a given blob versioned hash.
    #[serde(rename = "anvil_getBlobByHash", with = "sequence")]
    GetBlobByHash(B256),

    /// Returns the blobs for a given transaction hash.
    #[serde(rename = "anvil_getBlobsByTransactionHash", with = "sequence")]
    GetBlobByTransactionHash(TxHash),

    /// Returns the blobs for a given transaction hash.
    #[serde(rename = "anvil_getBlobSidecarsByBlockId", with = "sequence")]
    GetBlobSidecarsByBlockId(BlockId),

    /// Returns the genesis time for the chain
    #[serde(rename = "anvil_getGenesisTime", with = "empty_params")]
    GetGenesisTime(()),

    #[serde(rename = "eth_getTransactionByBlockHashAndIndex")]
    EthGetTransactionByBlockHashAndIndex(TxHash, Index),

    #[serde(rename = "eth_getTransactionByBlockNumberAndIndex")]
    EthGetTransactionByBlockNumberAndIndex(BlockNumberOrTag, Index),

    #[serde(rename = "eth_getRawTransactionByHash", with = "sequence")]
    EthGetRawTransactionByHash(TxHash),

    #[serde(rename = "eth_getRawTransactionByBlockHashAndIndex")]
    EthGetRawTransactionByBlockHashAndIndex(TxHash, Index),

    #[serde(rename = "eth_getRawTransactionByBlockNumberAndIndex")]
    EthGetRawTransactionByBlockNumberAndIndex(BlockNumberOrTag, Index),

    #[serde(rename = "eth_getTransactionReceipt", with = "sequence")]
    EthGetTransactionReceipt(B256),

    #[serde(rename = "eth_getBlockReceipts", with = "sequence")]
    EthGetBlockReceipts(BlockId),

    #[serde(rename = "eth_getUncleByBlockHashAndIndex")]
    EthGetUncleByBlockHashAndIndex(B256, Index),

    #[serde(rename = "eth_getUncleByBlockNumberAndIndex")]
    EthGetUncleByBlockNumberAndIndex(
        #[serde(deserialize_with = "lenient_block_number::lenient_block_number")] BlockNumberOrTag,
        Index,
    ),

    #[serde(rename = "eth_getLogs", with = "sequence")]
    EthGetLogs(Filter),

    /// Creates a filter object, based on filter options, to notify when the state changes (logs).
    #[serde(rename = "eth_newFilter", with = "sequence")]
    EthNewFilter(Filter),

    /// Polling method for a filter, which returns an array of logs which occurred since last poll.
    #[serde(rename = "eth_getFilterChanges", with = "sequence")]
    EthGetFilterChanges(String),

    /// Creates a filter in the node, to notify when a new block arrives.
    /// To check if the state has changed, call `eth_getFilterChanges`.
    #[serde(rename = "eth_newBlockFilter", with = "empty_params")]
    EthNewBlockFilter(()),

    /// Creates a filter in the node, to notify when new pending transactions arrive.
    /// To check if the state has changed, call `eth_getFilterChanges`.
    #[serde(rename = "eth_newPendingTransactionFilter", with = "empty_params")]
    EthNewPendingTransactionFilter(()),

    /// Returns an array of all logs matching filter with given id.
    #[serde(rename = "eth_getFilterLogs", with = "sequence")]
    EthGetFilterLogs(String),

    /// Removes the filter, returns true if the filter was installed
    #[serde(rename = "eth_uninstallFilter", with = "sequence")]
    EthUninstallFilter(String),

    #[serde(rename = "eth_getWork", with = "empty_params")]
    EthGetWork(()),

    #[serde(rename = "eth_submitWork")]
    EthSubmitWork(B64, B256, B256),

    #[serde(rename = "eth_submitHashrate")]
    EthSubmitHashRate(U256, B256),

    #[serde(rename = "eth_feeHistory")]
    EthFeeHistory(
        #[serde(deserialize_with = "deserialize_number")] U256,
        BlockNumberOrTag,
        #[serde(default)] Vec<f64>,
    ),

    #[serde(rename = "eth_syncing", with = "empty_params")]
    EthSyncing(()),

    #[serde(rename = "eth_config", with = "empty_params")]
    EthConfig(()),

    /// geth's `debug_getRawTransaction`  endpoint
    #[serde(rename = "debug_getRawTransaction", with = "sequence")]
    DebugGetRawTransaction(TxHash),

    /// geth's `debug_traceTransaction`  endpoint
    #[serde(rename = "debug_traceTransaction")]
    DebugTraceTransaction(B256, #[serde(default)] GethDebugTracingCallOptions),

    /// geth's `debug_traceCall`  endpoint
    #[serde(rename = "debug_traceCall")]
    DebugTraceCall(
        WithOtherFields<TransactionRequest>,
        #[serde(default)] Option<BlockId>,
        #[serde(default)] GethDebugTracingCallOptions,
    ),

    /// reth's `debug_codeByHash` endpoint
    #[serde(rename = "debug_codeByHash")]
    DebugCodeByHash(B256, #[serde(default)] Option<BlockId>),

    /// reth's `debug_dbGet` endpoint
    #[serde(rename = "debug_dbGet")]
    DebugDbGet(String),

    /// Trace transaction endpoint for parity's `trace_transaction`
    #[serde(rename = "trace_transaction", with = "sequence")]
    TraceTransaction(B256),

    /// Trace transaction endpoint for parity's `trace_block`
    #[serde(
        rename = "trace_block",
        deserialize_with = "lenient_block_number::lenient_block_number_seq"
    )]
    TraceBlock(BlockNumberOrTag),

    // Return filtered traces over blocks
    #[serde(rename = "trace_filter", with = "sequence")]
    TraceFilter(TraceFilter),
}
