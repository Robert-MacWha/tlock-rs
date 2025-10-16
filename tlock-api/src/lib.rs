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
    use alloy::transports::http::reqwest::Error;
    use serde::{Deserialize, Serialize};

    use crate::{RpcMethod, component::Component, entities::EntityId};

    /// Request the host registers a new entity with the given ID and this
    /// plugin as its owner.
    pub struct RegisterEntity;
    impl RpcMethod for RegisterEntity {
        const NAME: &'static str = "host_register_entity";
        type Params = EntityId;
        type Output = ();
    }

    /// Make a network request
    pub struct Fetch;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Request {
        pub url: String,
        pub method: String,
        pub headers: Vec<(String, Vec<u8>)>,
        pub body: Option<Vec<u8>>,
    }

    impl RpcMethod for Fetch {
        const NAME: &'static str = "host_fetch";
        type Params = Request;
        type Output = Result<Vec<u8>, String>;
    }

    /// Get the plugin's persistent state from the host.
    ///
    /// Returns `None` if no state has been stored.
    pub struct GetState;
    impl RpcMethod for GetState {
        const NAME: &'static str = "host_get_state";
        type Params = ();
        type Output = Option<Vec<u8>>;
    }

    /// Sets the plugin's persistent state to the host.
    pub struct SetState;
    impl RpcMethod for SetState {
        const NAME: &'static str = "host_set_state";
        type Params = Vec<u8>;
        type Output = ();
    }

    /// Sets an interface for a given interface ID.
    pub struct SetInterface;
    impl RpcMethod for SetInterface {
        const NAME: &'static str = "host_set_interface";
        type Params = (u32, Component); // (interface_id, component)
        type Output = ();
    }
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
    use alloy::rpc::types::{EthCallResponse, TransactionRequest, state::StateOverride};

    use crate::RpcMethod;

    pub struct BlockNumber;
    impl RpcMethod for BlockNumber {
        const NAME: &'static str = "eth_blockNumber";
        type Params = ();
        type Output = u64;
    }

    pub struct Call;
    impl RpcMethod for Call {
        const NAME: &'static str = "eth_call";
        type Params = (TransactionRequest, u64, Option<StateOverride>); // (tx, block_number, state_override)
        type Output = EthCallResponse;
    }

    pub struct GetBalance;
    impl RpcMethod for GetBalance {
        const NAME: &'static str = "eth_getBalance";
        type Params = (alloy::primitives::Address, u64); // (address, block_number)
        type Output = alloy::primitives::U256;
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
