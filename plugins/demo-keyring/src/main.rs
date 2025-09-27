use std::sync::Arc;

use log::info;
use tlock_pdk::{
    async_trait::async_trait,
    register_plugin,
    tlock_api::{
        CompositeClient,
        alloy_primitives::{Address, Signature, TxHash},
        domains::eip155_keyring::{
            CreateAccountParams, DeleteAccountParams, Eip155Keyring, Eip155KeyringServer,
            PersonalSignParams, SendTransactionParams, SignTypedDataParams,
        },
        domains::tlock::{TlockDomain, TlockDomainServer},
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
impl TlockDomain for DemoKeyring {
    type Error = RpcErrorCode;

    async fn ping(&self, message: String) -> Result<String, Self::Error> {
        info!("Plugin received ping with message: {}", message);
        let response = self
            .host
            .tlock()
            .ping(format!("Plugin Pong: {}", message))
            .await?;
        Ok(response)
    }

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

    async fn create_account(&self, params: CreateAccountParams) -> Result<Address, Self::Error> {
        info!("Creating account: {:?}", params);
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn delete_account(&self, params: DeleteAccountParams) -> Result<(), Self::Error> {
        info!("Deleting account: {:?}", params);
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn personal_sign(&self, params: PersonalSignParams) -> Result<Signature, Self::Error> {
        info!("Signing data: {:?}", params);
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn sign_typed_data(&self, params: SignTypedDataParams) -> Result<Signature, Self::Error> {
        info!("Signing typed data: {:?}", params);
        Err(RpcErrorCode::MethodNotFound)
    }

    async fn send_transaction(&self, params: SendTransactionParams) -> Result<TxHash, Self::Error> {
        info!("Sending tx: {:?}", params);
        Err(RpcErrorCode::MethodNotFound)
    }
}

register_plugin!(
    DemoKeyring,
    [Eip155KeyringServer::new, TlockDomainServer::new]
);
