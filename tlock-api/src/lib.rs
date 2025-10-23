use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use wasmi_pdk::{api::ApiError, rpc_message::RpcErrorCode, transport::Transport};

pub mod caip;
pub mod component;
pub mod domains;
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
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
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

macro_rules! rpc_method {
    (
        $(#[$meta:meta])*
        $name:ident, $struct_name:ident, $params:ty, $output:ty
    ) => {
        $(#[$meta])*
        #[allow(unused_doc_comments)]
        pub struct $struct_name;

        impl $crate::RpcMethod for $struct_name {
            type Params = $params;
            type Output = $output;

            const NAME: &'static str = stringify!($name);
        }
    };
}

/// The global namespace contains methods that are not specific to any particular
/// domain.
pub mod global {
    rpc_method!(
        /// Simple health check
        tlock_ping, Ping, (), String
    );
}

/// The host namespace contains methods for interacting with the host and
/// performing privileged operations.
pub mod host {
    use serde::{Deserialize, Serialize};

    use crate::{component::Component, entities::EntityId};

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Request {
        pub url: String,
        pub method: String,
        pub headers: Vec<(String, Vec<u8>)>,
        pub body: Option<Vec<u8>>,
    }

    rpc_method!(
        /// Request the host registers a new entity with the given ID and this
        /// plugin as its owner.
        host_register_entity, RegisterEntity, EntityId, ()
    );

    rpc_method!(
        /// Make a network request
        host_fetch, Fetch, Request, Result<Vec<u8>, String>
    );

    rpc_method!(
        /// Gets the plugin's persistent state from the host.
        ///
        /// Returns `None` if no state has been stored.
        host_get_state, GetState, (), Option<Vec<u8>>
    );

    rpc_method!(
        /// Sets the plugin's persistent state to the host.
        host_set_state, SetState, Vec<u8>, ()
    );

    rpc_method!(
        /// Sets an interface for a given interface ID.
        host_set_interface, SetInterface, (u32, Component), ()
    );
}

/// The plugin namespace contains methods implemented by plugins, generally
/// used by the host for lifecycle management.
pub mod plugin {
    /// Initialize the plugin, called by the host the first time a new plugin
    /// is registered.
    pub struct Init;
    impl super::RpcMethod for Init {
        const NAME: &'static str = "plugin_init";
        type Params = ();
        type Output = ();
    }
}

pub mod eth {
    use alloy::{
        eips::BlockId,
        primitives::Bytes,
        rpc::types::{BlockOverrides, TransactionRequest, state::StateOverride},
    };

    use crate::RpcMethod;

    pub struct BlockNumber;
    impl RpcMethod for BlockNumber {
        const NAME: &'static str = "eth_blockNumber";
        type Params = ();
        type Output = u64;
    }

    pub struct GetBalance;
    impl RpcMethod for GetBalance {
        const NAME: &'static str = "eth_getBalance";
        type Params = (alloy::primitives::Address, BlockId);
        type Output = alloy::primitives::U256;
    }

    pub struct Call;
    impl RpcMethod for Call {
        const NAME: &'static str = "eth_call";
        type Params = (
            TransactionRequest,
            Option<BlockOverrides>,
            Option<StateOverride>,
        );
        type Output = Bytes;
    }
}

/// The vault namespace contains methods for interacting with vaults, transferring
/// funds between different accounts.
pub mod vault {
    use alloy::primitives::U256;

    use crate::{
        RpcMethod,
        caip::{AccountId, AssetId},
        entities::VaultId,
    };

    /// Get the balance for all assets in a given account.
    pub struct GetAssets;
    impl RpcMethod for GetAssets {
        type Params = VaultId;
        type Output = Vec<(AssetId, U256)>;

        const NAME: &'static str = "vault_get_assets";
    }

    /// Withdraw an amount of some asset from this vault to another account.
    pub struct Withdraw;
    impl RpcMethod for Withdraw {
        type Params = (VaultId, AccountId, AssetId, U256); // (from, to, asset, amount)
        type Output = Result<(), String>;

        const NAME: &'static str = "vault_withdraw";
    }

    /// Gets the deposit address for a particular account and asset. Accounts can
    /// also use this to block deposits from unsupported assets or asset classes.
    ///
    /// Because vault implementations are black boxes, any plugin sending an asset
    /// to a vault MUST first call this method to ensure the asset is supported and
    /// the destination address is correct. Destination addresses may change over time,
    /// as might the supported assets.
    /// TODO: Consider making this automatic via the host? Not sure how.
    pub struct GetDepositAddress;
    impl RpcMethod for GetDepositAddress {
        type Params = (VaultId, AssetId); // (to, asset)
        type Output = Result<AccountId, String>;

        const NAME: &'static str = "vault_get_deposit_address";
    }

    /// Callback for when an amount is deposited in an account.
    /// TODO: Also strongly consider calling this automatically. Perhaps keep track
    /// of all the times `GetDepositAddress` is called during the execution of a
    /// plugin, and once complete call `OnDeposit` for each `GetDepositAddress` call.
    pub struct OnDeposit;
    impl RpcMethod for OnDeposit {
        type Params = (VaultId, AssetId); // (to, asset)
        type Output = ();

        const NAME: &'static str = "vault_on_deposit";
    }
}

pub mod page {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::RpcMethod;

    /// Called by the host when a registered page is loaded in the frontend. The
    /// plugin should setup any necessary interfaces with `host::SetInterface` here,
    /// dependant on the plugin's internal state.
    pub struct OnLoad;
    impl RpcMethod for OnLoad {
        type Params = u32; // (interface_id) // TODO Can I infer this interface id from the host? Perhaps, look into it.
        type Output = ();

        const NAME: &'static str = "page_on_load";
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum PageEvent {
        ButtonClicked(String),                               // (button_id)
        FormSubmitted(String, HashMap<String, Vec<String>>), // (form_id, form_values)
    }

    /// Called by the host when a registered page is updated in the frontend.
    pub struct OnUpdate;
    impl RpcMethod for OnUpdate {
        type Params = (u32, PageEvent); // (interface_id, event)
        type Output = ();

        const NAME: &'static str = "page_on_update";
    }
}
