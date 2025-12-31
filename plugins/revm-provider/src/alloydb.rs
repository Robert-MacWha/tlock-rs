use revm::{
    DatabaseRef,
    context::DBErrorMarker,
    primitives::{Address, B256},
    state::{AccountInfo, Bytecode},
};
use thiserror::Error;
use tlock_pdk::{
    tlock_api::{
        RpcMethod,
        alloy::{
            eips::BlockId,
            network::{Network, primitives::HeaderResponse},
            rpc::types::BlockTransactionsKind,
        },
        entities::EthProviderId,
        eth,
        rpc_batch::RpcBatch,
    },
    wasmi_plugin_pdk::{rpc_message::RpcError, transport::Transport},
};

/// Error type for transport-related database operations.
#[derive(Debug, Error)]
pub enum DBTransportError {
    #[error("RPC error: {0}")]
    RpcError(#[from] RpcError),
}

impl DBErrorMarker for DBTransportError {}

#[derive(Debug)]
pub struct AlloyDb<N: Network> {
    transport: Transport,
    provider_id: EthProviderId,
    block_number: BlockId,
    _marker: core::marker::PhantomData<fn() -> N>,
}

impl<N: Network> AlloyDb<N> {
    pub fn new(transport: Transport, provider_id: EthProviderId, block_number: BlockId) -> Self {
        Self {
            transport,
            provider_id,
            block_number,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<N: Network> DatabaseRef for AlloyDb<N> {
    type Error = DBTransportError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        type AccountStateBatch = (eth::GetTransactionCount, eth::GetBalance, eth::GetCode);
        let (nonce, balance, code) = AccountStateBatch::execute(
            self.transport.clone(),
            (
                (self.provider_id, address, self.block_number),
                (self.provider_id, address, self.block_number),
                (self.provider_id, address, self.block_number),
            ),
        )?;

        let code = Bytecode::new_raw(code.0.into());
        let code_hash = code.hash_slow();

        Ok(Some(AccountInfo::new(balance, nonce, code_hash, code)))
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        let block = eth::GetBlock.call(
            self.transport.clone(),
            (
                self.provider_id,
                BlockId::number(number),
                BlockTransactionsKind::Hashes,
            ),
        )?;

        Ok(B256::new(*block.header.hash()))
    }

    fn code_by_hash_ref(&self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        panic!("This should not be called, as the code is already loaded");
    }

    fn storage_ref(
        &self,
        address: Address,
        index: revm::primitives::StorageKey,
    ) -> Result<revm::primitives::StorageValue, Self::Error> {
        let storage_value = eth::GetStorageAt.call(
            self.transport.clone(),
            (self.provider_id, address, index, self.block_number),
        )?;

        Ok(storage_value)
    }
}
