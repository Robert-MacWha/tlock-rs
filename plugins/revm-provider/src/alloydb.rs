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
        alloy::{
            self,
            eips::{BlockId, BlockNumberOrTag},
            network::Network,
        },
        host,
    },
    wasmi_plugin_pdk::{rpc_message::RpcError, transport::Transport},
};

/// Error type for transport-related database operations.
#[derive(Debug, Error)]
pub enum DBTransportError {
    #[error("RPC error: {0}")]
    RpcError(#[from] RpcError),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Missing result in RPC response")]
    MissingResult,
}

impl DBErrorMarker for DBTransportError {}

#[derive(Debug)]
pub struct AlloyDb<N: Network> {
    transport: Transport,
    url: String,
    block_number: BlockId,
    _marker: core::marker::PhantomData<fn() -> N>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Deserialize)]
struct RpcResponse<T> {
    result: T,
}

impl<N: Network> AlloyDb<N> {
    pub fn new(transport: Transport, url: String, block_number: BlockId) -> Self {
        Self {
            transport,
            url,
            block_number,
            _marker: core::marker::PhantomData,
        }
    }

    fn execute_fetch<T: for<'a> Deserialize<'a>>(
        &self,
        payload: &Value,
    ) -> Result<T, DBTransportError> {
        let body = serde_json::to_vec(payload)?;

        let req = host::Request {
            url: self.url.clone(),
            method: "POST".to_string(),
            headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
            body: Some(body),
        };

        let resp = host::Fetch.call(self.transport.clone(), req)?;
        let resp = resp.map_err(RpcError::custom)?;
        Ok(serde_json::from_slice(&resp)?)
    }

    fn call_rpc<T: for<'a> Deserialize<'a>>(
        &self,
        method: &'static str,
        params: Vec<Value>,
    ) -> Result<T, DBTransportError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let response: JsonRpcResponse<T> = self.execute_fetch(&payload)?;

        if let Some(err) = response.error {
            return Err(DBTransportError::RpcError(RpcError::custom(format!(
                "RPC Error {}: {}",
                err.code, err.message
            ))));
        }

        response.result.ok_or(DBTransportError::MissingResult)
    }

    pub fn get_block(
        &self,
        number: BlockNumberOrTag,
    ) -> Result<alloy::rpc::types::Header, DBTransportError> {
        self.call_rpc(
            "eth_getBlockByNumber",
            vec![json!(number.to_string()), json!(false)],
        )
    }
}

impl<N: Network> DatabaseRef for AlloyDb<N> {
    type Error = DBTransportError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let block = json!(self.block_number);

        // 1. Define the batch request as an array
        let batch_payload = json!([
            {"jsonrpc": "2.0", "id": 2, "method": "eth_getBalance", "params": [address, block]},
            {"jsonrpc": "2.0", "id": 1, "method": "eth_getTransactionCount", "params": [address, block]},
            {"jsonrpc": "2.0", "id": 3, "method": "eth_getCode", "params": [address, block]}
        ]);

        // 2. Deserialize directly into a tuple of RpcResponse wrappers
        // revm primitives like U256 and Bytes already have hex-aware serde impls
        let (balance_res, nonce_res, code_res): (
            RpcResponse<U256>,
            RpcResponse<U64>,
            RpcResponse<Bytes>,
        ) = self.execute_fetch(&batch_payload)?;

        let code = Bytecode::new_raw(code_res.result);
        let code_hash = code.hash_slow();

        Ok(Some(AccountInfo::new(
            balance_res.result,
            nonce_res.result.to::<u64>(),
            code_hash,
            code,
        )))
    }

    fn storage_ref(
        &self,
        address: Address,
        index: revm::primitives::StorageKey,
    ) -> Result<revm::primitives::StorageValue, Self::Error> {
        self.call_rpc(
            "eth_getStorageAt",
            vec![json!(address), json!(index), json!(self.block_number)],
        )
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        #[derive(Deserialize)]
        struct Block {
            hash: B256,
        }
        let block: Block = self.call_rpc(
            "eth_getBlockByNumber",
            vec![json!(format!("{:#x}", number)), json!(false)],
        )?;
        Ok(block.hash)
    }

    fn code_by_hash_ref(&self, _: B256) -> Result<Bytecode, Self::Error> {
        unreachable!()
    }
}
