use alloy::rpc;
use revm::{
    DatabaseRef,
    context::DBErrorMarker,
    primitives::{Address, B256, Bytes, U256, alloy_primitives::U64},
    state::{AccountInfo, Bytecode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tlock_pdk::{
    tlock_api::{
        RpcMethod,
        alloy::{eips::BlockId, network::Network},
        host,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, RpcErrorContext},
        transport::Transport,
    },
};
use tracing::info;

#[derive(Debug, Error)]
pub enum AlloyDBError {
    #[error("RPC error: {0}")]
    RpcError(#[from] RpcError),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("RPC error: {message} (code: {code})")]
    JsonRpcError { message: String, code: i64 },
}

impl DBErrorMarker for AlloyDBError {}

#[derive(Debug)]
pub struct RemoteDB<N: Network> {
    transport: Transport,
    url: String,
    block_id: BlockId,
    _marker: core::marker::PhantomData<fn() -> N>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

impl<N: Network> RemoteDB<N> {
    pub fn new(transport: Transport, url: String, block_id: BlockId) -> Self {
        Self {
            transport,
            url,
            block_id,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<N: Network> DatabaseRef for RemoteDB<N> {
    type Error = AlloyDBError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        get_account_info(
            self.transport.clone(),
            self.url.clone(),
            self.block_id,
            address,
        )
    }

    fn storage_ref(
        &self,
        address: Address,
        index: revm::primitives::StorageKey,
    ) -> Result<revm::primitives::StorageValue, Self::Error> {
        get_storage(
            self.transport.clone(),
            self.url.clone(),
            self.block_id,
            address,
            index,
        )
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        get_block_hash(self.transport.clone(), self.url.clone(), number)
    }

    fn code_by_hash_ref(&self, _: B256) -> Result<Bytecode, Self::Error> {
        return Err(AlloyDBError::RpcError(RpcError::custom(
            "code_by_hash_ref is not supported in RemoteDB",
        )));
    }
}

pub fn get_chain_id(transport: Transport, url: String) -> Result<u64, AlloyDBError> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_chainId",
        "params": []
    });

    let chain_id: U64 = rpc_call(transport, url, payload)?;
    Ok(chain_id.to::<u64>())
}

pub fn get_latest_block_header(
    transport: Transport,
    url: String,
) -> Result<rpc::types::Header, AlloyDBError> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getBlockByNumber",
        "params": ["latest", false]
    });

    let block: rpc::types::Block = rpc_call(transport, url, payload)?;
    Ok(block.header)
}

fn get_account_info(
    transport: Transport,
    url: String,
    block_id: BlockId,
    address: Address,
) -> Result<Option<AccountInfo>, AlloyDBError> {
    let block = json!(block_id);

    let batch = json!([
        {"jsonrpc": "2.0", "id": 1, "method": "eth_getBalance", "params": [address, block]},
        {"jsonrpc": "2.0", "id": 2, "method": "eth_getTransactionCount", "params": [address, block]},
        {"jsonrpc": "2.0", "id": 3, "method": "eth_getCode", "params": [address, block]}
    ]);

    let (balance, nonce, code): (
        JsonRpcResponse<U256>,
        JsonRpcResponse<U64>,
        JsonRpcResponse<Bytes>,
    ) = rpc_batch(transport, url, batch)?;

    let balance = balance.result.context(format!(
        "Failed to get balance for account {:?}: {:?}",
        address, balance.error
    ))?;
    let nonce = nonce.result.context(format!(
        "Failed to get nonce for account {:?}: {:?}",
        address, nonce.error
    ))?;
    let code = code.result.context(format!(
        "Failed to get code for account {:?}: {:?}",
        address, code.error
    ))?;

    let bytecode = Bytecode::new_raw(code);
    let code_hash = bytecode.hash_slow();

    Ok(Some(AccountInfo::new(
        balance,
        nonce.to::<u64>(),
        code_hash,
        bytecode,
    )))
}

fn get_storage(
    transport: Transport,
    url: String,
    block_id: BlockId,
    address: Address,
    index: revm::primitives::StorageKey,
) -> Result<revm::primitives::StorageValue, AlloyDBError> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getStorageAt",
        "params": [address, index, block_id]
    });

    rpc_call(transport, url, payload)
}

fn get_block_hash(transport: Transport, url: String, number: u64) -> Result<B256, AlloyDBError> {
    #[derive(Deserialize)]
    struct Block {
        hash: B256,
    }

    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getBlockByNumber",
        "params": [format!("{:#x}", number), false]
    });

    let block: Block = rpc_call(transport, url, payload)?;
    Ok(block.hash)
}

fn rpc_call<T: for<'a> Deserialize<'a>>(
    transport: Transport,
    url: String,
    payload: Value,
) -> Result<T, AlloyDBError> {
    let body = serde_json::to_vec(&payload)?;

    info!("Cache Miss - RPC Call: {}", payload);

    let req = host::Request {
        url,
        method: "POST".to_string(),
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: Some(body),
    };

    let resp = host::Fetch.call(transport, req)?;
    let resp = resp.map_err(RpcError::custom)?;

    let response: JsonRpcResponse<T> = serde_json::from_slice(&resp)?;

    if let Some(err) = response.error {
        return Err(AlloyDBError::JsonRpcError {
            message: err.message,
            code: err.code,
        });
    }

    Ok(response
        .result
        .context("Missing result in JSON-RPC response")?)
}

fn rpc_batch<T: for<'a> Deserialize<'a>>(
    transport: Transport,
    url: String,
    payload: Value,
) -> Result<T, AlloyDBError> {
    let body = serde_json::to_vec(&payload)?;

    let req = host::Request {
        url,
        method: "POST".to_string(),
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: Some(body),
    };

    let resp = host::Fetch.call(transport, req)?;
    let resp = resp.map_err(RpcError::custom)?;

    // Batch responses are returned directly as an array, no wrapper
    Ok(serde_json::from_slice(&resp)?)
}
