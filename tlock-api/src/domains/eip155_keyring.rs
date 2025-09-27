use crate::{methods::Methods, routes::Eip155AccountRoute, routes::PluginIdRoute};
use alloy_dyn_abi::TypedData;
use alloy_primitives::{Address, Bytes, ChainId, Signature, TxHash};
use alloy_rpc_types::TransactionRequest;
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use serde::{Deserialize, Serialize};
use wasmi_pdk::api::ApiError;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountParams {
    pub route: PluginIdRoute,
    pub chain_id: ChainId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteAccountParams {
    pub route: Eip155AccountRoute,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PersonalSignParams {
    pub route: Eip155AccountRoute,
    pub data: Bytes,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataParams {
    pub route: Eip155AccountRoute,
    pub data: TypedData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendTransactionParams {
    pub route: Eip155AccountRoute,
    pub tx: TransactionRequest,
}

#[rpc_namespace]
#[async_trait]
/// The eip155 keyring domain allows a plugin to create and manage account entities.
pub trait Eip155Keyring: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::KeyringCreateAccount)]
    /// Creates the account
    ///
    /// Routed via the plugin ID
    async fn create_account(&self, params: CreateAccountParams) -> Result<Address, Self::Error>;

    #[rpc_method(Methods::KeyringDeleteAccount)]
    /// Deletes the account
    ///
    /// Routed via the account's caip-10 id
    async fn delete_account(&self, params: DeleteAccountParams) -> Result<(), Self::Error>;

    #[rpc_method(Methods::PersonalSign)]
    /// Calculates an Ethereum specific signature with
    /// `sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))`.
    /// Adding a prefix to the message makes the calculated signature recognizable
    /// as an Ethereum specific signature.
    ///
    /// https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-personal#personalsign
    ///
    /// Routed via the account's caip-10 id
    async fn personal_sign(&self, params: PersonalSignParams) -> Result<Signature, Self::Error>;

    #[rpc_method(Methods::EthSignTypedData)]
    /// Signs typed data (EIP-712).
    ///
    /// https://eips.ethereum.org/EIPS/eip-712
    ///
    /// Routed via the account's caip-10 id
    async fn sign_typed_data(&self, params: SignTypedDataParams) -> Result<Signature, Self::Error>;

    #[rpc_method(Methods::EthSendTransaction)]
    /// Creates a transaction and sends it to the network.
    ///
    /// https://ethereum.org/developers/docs/apis/json-rpc/#eth_sendtransaction
    ///
    /// Routed via the account's caip-10 id
    async fn send_transaction(&self, params: SendTransactionParams) -> Result<TxHash, Self::Error>;
}
