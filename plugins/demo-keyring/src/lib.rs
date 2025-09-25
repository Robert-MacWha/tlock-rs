use std::sync::Arc;

use log::info;
use tlock_pdk::{
    async_trait::{self, async_trait},
    register_plugin,
    tlock_api::{
        CompositeClient,
        alloy_dyn_abi::TypedData,
        alloy_primitives::{Address, Bytes, ChainId, Signature, TxHash},
        alloy_rpc_types::TransactionRequest,
        eip155_keyring::{Eip155Keyring, Eip155KeyringServer},
        plugin::{PluginNamespace, PluginNamespaceServer},
    },
    wasmi_pdk::rpc_message::RpcErrorCode,
};

struct DemoKeyring {
    host: Arc<CompositeClient<RpcErrorCode>>,
}

impl DemoKeyring {
    pub fn new(host: Arc<CompositeClient<RpcErrorCode>>) -> Self {
        Self { host }
    }
}

#[async_trait]
impl PluginNamespace for DemoKeyring {
    type Error = RpcErrorCode;

    async fn name(&self) -> Result<String, Self::Error> {
        Ok("demo-keyring".to_string())
    }

    async fn version(&self) -> Result<String, Self::Error> {
        Ok("0.1.0".to_string())
    }
}

#[async_trait]
impl Eip155Keyring for DemoKeyring {
    type Error = RpcErrorCode;

    async fn create_account(&self, chain_id: ChainId) -> Result<Address, Self::Error> {
        info!("Creating account for chain_id: {}", chain_id);
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn delete_account(&self, address: Address) -> Result<(), Self::Error> {
        info!("Deleting account: {}", address);
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn get_accounts(&self) -> Result<Vec<Address>, Self::Error> {
        info!("Getting accounts");
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn personal_sign(&self, data: Bytes) -> Result<Signature, Self::Error> {
        info!("Signing data");
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn sign_typed_data(&self, data: TypedData) -> Result<Signature, Self::Error> {
        info!("Signing typed data");
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn send_transaction(&self, tx: TransactionRequest) -> Result<TxHash, Self::Error> {
        info!("Sending transaction");
        Err(RpcErrorCode::MethodNotFound)
    }
}

register_plugin!(
    DemoKeyring,
    [Eip155KeyringServer::new, PluginNamespaceServer::new]
);
