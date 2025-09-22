use alloy_dyn_abi::TypedData;
use alloy_primitives::{Address, Bytes, ChainId, Signature, TxHash};
use alloy_rpc_types::TransactionRequest;
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use wasmi_pdk::api::ApiError;

use crate::methods::Methods;

#[rpc_namespace]
#[async_trait]
pub trait Eip155Keyring: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::KeyringCreateAccount)]
    async fn create_account(&self, chain: ChainId) -> Result<Address, Self::Error>;

    #[rpc_method(Methods::EthAccounts)]
    async fn accounts(&self, chain: ChainId) -> Result<Vec<Address>, Self::Error>;

    #[rpc_method(Methods::KeyringDeleteAccount)]
    async fn delete_account(&self, address: Address) -> Result<(), Self::Error>;

    #[rpc_method(Methods::PersonalSign)]
    async fn personal_sign(&self, data: Bytes, address: Address) -> Result<Signature, Self::Error>;

    #[rpc_method(Methods::EthSignTypedDataV4)]
    async fn eth_sign_typed_data_v4(
        &self,
        address: Address,
        data: TypedData,
    ) -> Result<Signature, Self::Error>;

    #[rpc_method(Methods::EthSendRawTransaction)]
    async fn eth_send_raw_transaction(&self, tx: Bytes) -> Result<TxHash, Self::Error>;

    #[rpc_method(Methods::EthSendTransaction)]
    async fn eth_send_transaction(&self, tx: TransactionRequest) -> Result<TxHash, Self::Error>;
}
