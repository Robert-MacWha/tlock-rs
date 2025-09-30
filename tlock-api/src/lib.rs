use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use wasmi_pdk::{api::ApiError, rpc_message::RpcErrorCode, transport::Transport};

pub mod caip;
pub mod entities;

// TODO: Consider adding a `mod sealed::Sealed {}` to prevent external impl, forcing
// plugins to only use provided methods.
//
// That's already somewhat enforced since the host will only call / recognize
// these methods, but could be nice to make it explicit.
// TODO: Or alternatively, perhaps move it into the `wasmi_pdk` crate, since
// it should work fine for any RPC system.
// TODO: Also consider forwards compatibility with associated types, maybe wrap
// them as named structs to allow adding fields later without introducing breaking changes.
#[async_trait]
pub trait RpcMethod: Send + Sync {
    type Params: DeserializeOwned + Serialize + Send + Sync;
    type Output: DeserializeOwned + Serialize + Send + Sync;

    const NAME: &'static str;

    /// Call this RPC method on the given transport with the provided params.
    async fn call<E, T>(&self, transport: Arc<T>, params: Self::Params) -> Result<Self::Output, E>
    where
        E: ApiError,
        T: Transport<E> + Send + Sync + 'static,
    {
        let raw_params =
            serde_json::to_value(params).map_err(|_| RpcErrorCode::InvalidParams.into())?;
        let resp = transport.call(Self::NAME, raw_params).await?;
        let result =
            serde_json::from_value(resp.result).map_err(|_| RpcErrorCode::InternalError.into())?;
        Ok(result)
    }
}

/// The global namespace contains methods that are not specific to any particular
/// domain.
pub mod global {
    use super::RpcMethod;

    /// Simple health check
    pub struct Ping;
    impl RpcMethod for Ping {
        type Params = ();
        type Output = String;

        const NAME: &'static str = "tlock_ping";
    }
}

/// The host namespace contains methods for interacting with the host and
/// performing privileged operations.
pub mod host {
    use crate::{RpcMethod, entities::EntityId};

    /// Request the host creates a new entity with the given ID and this
    /// plugin as its owner.
    pub struct CreateEntity;

    impl RpcMethod for CreateEntity {
        type Params = EntityId;
        type Output = ();

        const NAME: &'static str = "host_create_entity";
    }
}

/// The vault namespace contains methods for interacting with vaults, transferring
/// funds between different accounts.
pub mod vault {
    use alloy_primitives::U256;

    use crate::{
        RpcMethod,
        caip::{AccountId, AssetId},
        entities::VaultId,
    };

    /// Get the balance for all assets in a given account.
    pub struct BalanceOf;
    impl RpcMethod for BalanceOf {
        type Params = VaultId;
        type Output = Vec<(AssetId, U256)>;

        const NAME: &'static str = "vault_balance_of";
    }

    /// Transfer an amount from one account to another.
    pub struct Transfer;
    impl RpcMethod for Transfer {
        type Params = (VaultId, AccountId, AssetId, U256); // (from, to, asset, amount)
        type Output = Result<(), String>;

        const NAME: &'static str = "vault_transfer";
    }

    /// Gets the receipt address for a particular account and asset. Accounts can
    /// also use this to block deposits from unsupported assets or asset classes.
    ///  
    /// Because vault implementations are black boxes, any plugin sending an asset
    /// to a vault MUST first call this method to ensure the asset is supported and
    /// the destination address is correct. Destination addresses may change over time,
    /// as might the supported assets.
    pub struct GetReceiptAddress;
    impl RpcMethod for GetReceiptAddress {
        type Params = (VaultId, AssetId); // (to, asset)
        type Output = Result<AccountId, String>;

        const NAME: &'static str = "vault_get_receipt_address";
    }

    /// Receive an amount in an account. It is called by the host after a transfer
    /// has been confirmed.
    pub struct OnReceive;
    impl RpcMethod for OnReceive {
        type Params = (VaultId, AssetId); // (to, amount)
        type Output = ();

        const NAME: &'static str = "vault_receive";
    }
}
