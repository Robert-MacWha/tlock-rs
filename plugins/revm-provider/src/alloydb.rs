use core::error::Error;
use std::fmt::Display;

use revm::{
    DatabaseRef,
    context::DBErrorMarker,
    primitives::{Address, B256},
    state::{AccountInfo, Bytecode},
};
use tlock_pdk::tlock_api::alloy::{
    eips::BlockId,
    network::{BlockResponse, Network, primitives::HeaderResponse},
    providers::Provider,
    transports::TransportError,
};
use tokio::runtime::Handle;

/// Error type for transport-related database operations.
#[derive(Debug)]
pub struct DBTransportError(pub TransportError);

impl DBErrorMarker for DBTransportError {}

impl Display for DBTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Transport error: {}", self.0)
    }
}

impl Error for DBTransportError {}

impl From<TransportError> for DBTransportError {
    fn from(e: TransportError) -> Self {
        Self(e)
    }
}

pub struct AlloyDb<N: Network, P: Provider<N>> {
    provider: P,
    block_number: BlockId,
    handle: Handle,
    _marker: core::marker::PhantomData<fn() -> N>,
}

impl<N: Network, P: Provider<N>> AlloyDb<N, P> {
    /// Creates a new `AlloyDb` instance.
    ///
    /// Returns `None` if the current thread does not have a Tokio runtime.
    pub fn new(provider: P, block_number: BlockId) -> Option<Self> {
        let handle = Handle::try_current().ok()?;

        Some(Self {
            provider,
            block_number,
            handle,
            _marker: core::marker::PhantomData,
        })
    }

    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.handle.block_on(fut)
    }
}

impl<N: Network, P: Provider<N>> DatabaseRef for AlloyDb<N, P> {
    type Error = DBTransportError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.block_on(async {
            let (nonce, balance, code) = tokio::try_join!(
                self.provider
                    .get_transaction_count(address)
                    .block_id(self.block_number),
                self.provider
                    .get_balance(address)
                    .block_id(self.block_number),
                self.provider
                    .get_code_at(address)
                    .block_id(self.block_number),
            )?;

            let code = Bytecode::new_raw(code.0.into());
            let code_hash = code.hash_slow();

            Ok(Some(AccountInfo::new(balance, nonce, code_hash, code)))
        })
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.block_on(async {
            let block = self.provider.get_block_by_number(number.into()).await?;
            Ok(B256::new(*block.unwrap().header().hash()))
        })
    }

    fn code_by_hash_ref(&self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        panic!("This should not be called, as the code is already loaded");
    }

    fn storage_ref(
        &self,
        address: Address,
        index: revm::primitives::StorageKey,
    ) -> Result<revm::primitives::StorageValue, Self::Error> {
        self.block_on(async {
            Ok(self
                .provider
                .get_storage_at(address, index)
                .block_id(self.block_number)
                .await?)
        })
    }
}
