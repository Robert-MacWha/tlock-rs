use alloy_primitives::{Address, ChainId};
use async_trait::async_trait;
use wasmi_pdk::api::ApiError;

#[async_trait]
pub trait Eip155Keyring<E: ApiError> {
    async fn create_account(chain: ChainId) -> Result<Address, E>;
    async fn list_accounts(chain: ChainId) -> Result<Vec<Address>, E>;
}
