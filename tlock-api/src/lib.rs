use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use wasmi_pdk::{api::ApiError, rpc_message::RpcError, transport::Transport};

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
        let raw_params = serde_json::to_value(params).map_err(|_| RpcError::InvalidParams)?;
        let resp = transport.call(Self::NAME, raw_params).await?;
        let result = serde_json::from_value(resp.result).map_err(|_| RpcError::InternalError)?;
        Ok(result)
    }
}

macro_rules! rpc_method {
    (
        $(#[$meta:meta])*
        $name:ident, $struct_name:ident, $params:ty, $output:ty
    ) => {
        $(#[$meta])*
        ///
        #[doc = concat!("**Params:** `", stringify!($params), "`")]
        #[doc = concat!("**Output:** `", stringify!($output), "`")]
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
    use crate::{
        component::Component,
        domains::Domain,
        entities::{EntityId, PageId},
    };
    use serde::{Deserialize, Serialize};

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
        host_register_entity, RegisterEntity, Domain, EntityId
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
        /// Sets a specific page to the given component.
        host_set_page, SetInterface, (PageId, Component), ()
    );
}

/// The plugin namespace contains methods implemented by plugins, generally
/// used by the host for lifecycle management.
pub mod plugin {
    rpc_method!(
        /// Initialize the plugin, called by the host the first time a new plugin
        /// is registered.
        plugin_init, Init, (), ()
    );
}

pub mod eth {
    use alloy::{
        eips::BlockId,
        primitives::{Address, Bytes, TxHash},
        rpc::types::{
            Block, BlockOverrides, BlockTransactionsKind, Filter, Log, Transaction,
            TransactionReceipt, TransactionRequest, state::StateOverride,
        },
    };

    use crate::entities::EthProviderId;

    rpc_method!(
        /// Get the current block number.
        eth_block_number, BlockNumber, EthProviderId, u64
    );

    rpc_method!(
        /// Executes a new message call immediately without creating a transaction on the block chain.
        eth_call, Call, (EthProviderId, TransactionRequest, Option<BlockOverrides>, Option<StateOverride>), Bytes
    );

    rpc_method!(
        /// Gets the current gas price.
        eth_gas_price, GasPrice, EthProviderId, alloy::primitives::U256
    );

    rpc_method!(
        /// Gets the balance of an address at a given block.
        eth_get_balance, GetBalance, (EthProviderId, alloy::primitives::Address, BlockId), alloy::primitives::U256
    );

    rpc_method!(
        /// Gets a block by its hash or number.
        eth_get_block, GetBlock, (EthProviderId, BlockId, BlockTransactionsKind), Block
    );

    rpc_method!(
        /// Gets a block receipt by its hash or number.
        eth_get_block_receipts, GetBlockReceipts, (EthProviderId, BlockId), Vec<TransactionReceipt>
    );

    rpc_method!(
        // Gets logs matching the given filter object.
        eth_get_logs,
        GetLogs,
        (EthProviderId, Filter),
        Vec<Log>
    );

    rpc_method!(
        /// Gets the compiled bytecode of a smart contract.
        eth_get_code, GetCode, (EthProviderId, Address, BlockId), Bytes
    );

    rpc_method!(
        /// Gets a transaction by its hash
        eth_get_transaction_by_hash, GetTransactionByHash, (EthProviderId, TxHash), Transaction
    );

    rpc_method!(
        /// Gets a transaction receipt by its hash
        eth_get_transaction_receipt, GetTransactionReceipt, (EthProviderId, TxHash), TransactionReceipt
    );

    // TODO: Consider making this a different domain and having a distinction between "eth-read" and "eth-write"
    // methods. Would also make it easier to add custom send methods (IE to private pool, or forwarding to devp2p, etc).
    rpc_method!(
        /// Sends a raw transaction to the network.
        eth_send_raw_transaction, SendRawTransaction, (EthProviderId, Bytes), TxHash
    );
}

/// The vault namespace contains methods for interacting with vaults, transferring
/// funds between different accounts.
pub mod vault {
    use crate::{
        caip::{AccountId, AssetId},
        entities::VaultId,
    };
    use alloy::primitives::U256;

    rpc_method!(
        /// Get the balance for all assets in a given account.
        vault_get_assets, GetAssets, VaultId, Vec<(AssetId, U256)>
    );

    rpc_method!(
        /// Withdraw an amount of some asset from this vault to another account.
        vault_withdraw, Withdraw, (VaultId, AccountId, AssetId, U256), Result<(), String>
    );

    rpc_method!(
        /// Gets the deposit address for a particular account and asset. Accounts can
        /// also use this to block deposits from unsupported assets or asset classes.
        ///
        /// Because vault implementations are black boxes, any plugin sending an asset
        /// to a vault MUST first call this method to ensure the asset is supported and
        /// the destination address is correct. Destination addresses may change over time,
        /// as might the supported assets.
        ///
        /// TODO: Consider making this automatic via the host? Not sure how.
        vault_get_deposit_address, GetDepositAddress, (VaultId, AssetId), Result<AccountId, String>
    );

    rpc_method!(
        /// Callback for when an amount is deposited in an account.
        /// TODO: Also strongly consider calling this automatically. Perhaps keep track
        /// of all the times `GetDepositAddress` is called during the execution of a
        /// plugin, and once complete call `OnDeposit` for each `GetDepositAddress` call.
        vault_on_deposit, OnDeposit, (VaultId, AssetId), ()
    );
}

pub mod page {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use crate::entities::PageId;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum PageEvent {
        ButtonClicked(String),                               // (button_id)
        FormSubmitted(String, HashMap<String, Vec<String>>), // (form_id, form_values)
    }

    rpc_method!(
        /// Called by the host when a registered page is loaded in the frontend. The
        /// plugin should setup any necessary interfaces with `host::SetInterface` here,
        /// dependant on the plugin's internal state.
        page_on_load, OnLoad, PageId, ()
    );

    rpc_method!(
        /// Called by the host when a registered page is updated in the frontend.
        page_on_update, OnUpdate, (PageId, PageEvent), ()
    );
}
