use alloy_dyn_abi::TypedData;
use alloy_primitives::{Address, Bytes, ChainId, Signature, TxHash};
use alloy_rpc_types::TransactionRequest;
use async_trait::async_trait;
use rpc_namespace::{rpc_method, rpc_namespace};
use wasmi_pdk::api::ApiError;

use crate::methods::Methods;

#[rpc_namespace]
#[async_trait]
/// The keyring domain allows you to manage many eip155 eoa-style accounts to sign messages
/// and transactions.
///
/// When the user creates an account, the keyring can derive the account's address and
/// return it to tlock. From there, tlock will route any signing requests for that caip-10 account
/// to the keyring.
pub trait Eip155Keyring: Send + Sync {
    type Error: ApiError;

    #[rpc_method(Methods::KeyringCreateAccount)]
    /// Creates the account
    async fn create_account(&self, chain_id: ChainId) -> Result<Address, Self::Error>;

    #[rpc_method(Methods::KeyringDeleteAccount)]
    /// Deletes the account
    async fn delete_account(&self, address: Address) -> Result<(), Self::Error>;

    #[rpc_method(Methods::EthAccounts)]
    /// Returns the account address
    async fn get_accounts(&self) -> Result<Vec<Address>, Self::Error>;

    #[rpc_method(Methods::PersonalSign)]
    /// Calculates an Ethereum specific signature with
    /// `sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))`.
    /// Adding a prefix to the message makes the calculated signature recognizable
    /// as an Ethereum specific signature.
    ///
    /// https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-personal#personalsign
    async fn personal_sign(&self, data: Bytes) -> Result<Signature, Self::Error>;

    #[rpc_method(Methods::EthSignTypedData)]
    /// Signs typed data (EIP-712).
    ///
    /// https://eips.ethereum.org/EIPS/eip-712
    async fn sign_typed_data(&self, data: TypedData) -> Result<Signature, Self::Error>;

    #[rpc_method(Methods::EthSendTransaction)]
    /// Creates a transaction and sends it to the network.
    ///
    /// https://ethereum.org/developers/docs/apis/json-rpc/#eth_sendtransaction
    async fn send_transaction(&self, tx: TransactionRequest) -> Result<TxHash, Self::Error>;
}
