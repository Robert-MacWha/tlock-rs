use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use wasmi_pdk::{api::ApiError, rpc_message::RpcErrorCode, transport::Transport};

pub mod caip;
pub mod entities;
pub use alloy_dyn_abi;
pub use alloy_primitives;
pub use alloy_rpc_types;
pub mod component;
pub mod domains;

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
    use crate::{RpcMethod, component::Component, entities::EntityId};

    /// Request the host registers a new entity with the given ID and this
    /// plugin as its owner.
    pub struct RegisterEntity;

    impl RpcMethod for RegisterEntity {
        type Params = EntityId;
        type Output = ();

        const NAME: &'static str = "host_register_entity";
    }

    /// Get the plugin's persistent state from the host.
    ///
    /// Returns `None` if no state has been stored.
    pub struct GetState;
    impl RpcMethod for GetState {
        type Params = ();
        type Output = Option<Vec<u8>>;

        const NAME: &'static str = "host_get_state";
    }

    /// Sets the plugin's persistent state to the host.
    pub struct SetState;
    impl RpcMethod for SetState {
        type Params = Vec<u8>;
        type Output = ();

        const NAME: &'static str = "host_set_state";
    }

    /// Sets an interface for a given interface ID.
    pub struct SetInterface;
    impl RpcMethod for SetInterface {
        type Params = (u32, Component); // (interface_id, component)
        type Output = ();

        const NAME: &'static str = "host_set_interface";
    }
}

/// The plugin namespace contains methods implemented by plugins, generally
/// used by the host for lifecycle management.
pub mod plugin {
    /// Initialize the plugin, called by the host the first time a new plugin
    /// is registered.
    pub struct Init;
    impl super::RpcMethod for Init {
        type Params = ();
        type Output = ();

        const NAME: &'static str = "plugin_init";
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

    /// Transfer an amount of some asset from this vault to another account.
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
    /// TODO: Consider making this automatic via the host? Not sure how.
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

pub mod page {
    use serde::{Deserialize, Serialize};

    use crate::RpcMethod;

    /// Called by the host when a registered page is loaded in the frontend. The
    /// plugin should setup any necessary interfaces with `host::SetInterface` here,
    /// dependant on the plugin's internal state.
    pub struct OnPageLoad;
    impl RpcMethod for OnPageLoad {
        type Params = u32; // (interface_id) // TODO Can I infer this interface id from the host? Perhaps, look into it.
        type Output = ();

        const NAME: &'static str = "page_on_load";
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum PageEvent {
        ButtonClicked(u32), // (button_id)
    }

    /// Called by the host when a registered page is updated in the frontend.
    pub struct OnPageUpdate;
    impl RpcMethod for OnPageUpdate {
        type Params = (u32, PageEvent); // (interface_id, event)
        type Output = ();

        const NAME: &'static str = "page_on_update";
    }
}
