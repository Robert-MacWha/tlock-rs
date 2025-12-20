use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use wasmi_plugin_pdk::{api::ApiError, rpc_message::RpcError, transport::Transport};

pub mod caip;
pub mod component;
pub mod domains;
pub mod entities;
pub use alloy;

// TODO: Add a signer trait just for signing raw messages? Not sure if it'd work
// - we might end up with too many types requiring user authentication.

// TODO: Consider adding a `mod sealed::Sealed {}` to prevent external impl,
// forcing plugins to only use provided methods. That's already somewhat
// enforced since the host will only call / recognize these methods, but could
// be nice to make it explicit. Or alternatively, perhaps move it into the
// `wasmi_plugin_pdk` crate, since it should work fine for any RPC system.

// TODO: Also consider forwards compatibility with associated types, maybe wrap
// them as named structs to allow adding fields later without introducing
// breaking changes.
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
        let result = serde_json::from_value(resp.result)
            .map_err(|_| RpcError::Custom("Deserialization Error".into()))?;
        Ok(result)
    }
}

macro_rules! rpc_method {
    (
        $(#[$meta:meta])*
        $name:ident, $struct_name:ident, $params:ty, $output:ty
    ) => {
        $(#[$meta])*
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

/// The global namespace contains methods that are not specific to any
/// particular domain.
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

    use crate::{
        caip::ChainId,
        component::Component,
        domains::Domain,
        entities::{CoordinatorId, EntityId, EthProviderId, PageId, VaultId},
    };

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

    // TODO: Consider turning the host_request_* into their own domain? Makes it
    // more obvious they're all related and can share docs.
    rpc_method!(
        /// Request the host to provide an EthProvider for this plugin
        host_request_eth_provider,
        RequestEthProvider,
        ChainId,
        EthProviderId
    );

    rpc_method!(
        /// Request the host to provide a Vault for this plugin
        host_request_vault, RequestVault, (), VaultId
    );

    rpc_method!(
        /// Requests the host to provide a Coordinator for this plugin
        host_request_coordinator,
        RequestCoordinator,
        (),
        CoordinatorId
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
        host_set_page, SetPage, (PageId, Component), ()
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

/// The eth namespace contains methods for interacting with EVM chains.
/// It aims to be fully compatible with standard Ethereum JSON-RPC methods.
pub mod eth {
    use alloy::{
        eips::BlockId,
        primitives::{Address, Bytes, TxHash, U256},
        rpc::types::{
            Block, BlockOverrides, BlockTransactionsKind, Filter, Log, Transaction,
            TransactionReceipt, TransactionRequest, state::StateOverride,
        },
    };

    use crate::entities::EthProviderId;

    rpc_method!(
        /// Get the current block number.
        eth_blockNumber, BlockNumber, EthProviderId, u64
    );

    rpc_method!(eth_chainId, ChainId, EthProviderId, U256);

    rpc_method!(
        /// Executes a new message call immediately without creating a transaction on the block chain.
        eth_call, Call, (EthProviderId, TransactionRequest, BlockId, Option<StateOverride>, Option<BlockOverrides>), Bytes
    );

    rpc_method!(
        /// Gets the current gas price.
        eth_gasPrice, GasPrice, EthProviderId, u128
    );

    rpc_method!(
        /// Gets the balance of an address at a given block.
        eth_getBalance, GetBalance, (EthProviderId, alloy::primitives::Address, BlockId), alloy::primitives::U256
    );

    rpc_method!(
        /// Gets a block by its hash or number.
        eth_getBlock, GetBlock, (EthProviderId, BlockId, BlockTransactionsKind), Block
    );

    rpc_method!(
        /// Gets a block receipt by its hash or number.
        eth_getBlockReceipts, GetBlockReceipts, (EthProviderId, BlockId), Vec<TransactionReceipt>
    );

    rpc_method!(
        // Gets logs matching the given filter object.
        eth_getLogs,
        GetLogs,
        (EthProviderId, Filter),
        Vec<Log>
    );
    rpc_method!(
        /// Gets the compiled bytecode of a smart contract.
        eth_getCode, GetCode, (EthProviderId, Address, BlockId), Bytes
    );

    rpc_method!(
        /// Gets a transaction by its hash
        eth_getTransactionByHash, GetTransactionByHash, (EthProviderId, TxHash), Transaction
    );

    rpc_method!(
        /// Gets a transaction receipt by its hash
        eth_getTransactionReceipt, GetTransactionReceipt, (EthProviderId, TxHash), TransactionReceipt
    );

    rpc_method!(
        /// Gets the transaction count (AKA "nonce") for an address at a given block.
        eth_getTransactionCount, GetTransactionCount, (EthProviderId, Address, BlockId), u64
    );

    rpc_method!(
        /// Estimates the gas necessary to complete a transaction.
        eth_estimateGas, EstimateGas, (EthProviderId, TransactionRequest, BlockId, Option<StateOverride>, Option<BlockOverrides>), u64
    );

    // TODO: Consider making this a different domain and having a distinction
    // between "eth-read" and "eth-write" methods. Would also make it easier to
    // add custom send methods (IE to private pool, or forwarding to devp2p, etc).
    rpc_method!(
        /// Sends a raw transaction to the network.
        eth_sendRawTransaction, SendRawTransaction, (EthProviderId, Bytes), TxHash
    );
}

/// The vault namespace contains methods for interacting with vaults,
/// transferring funds between different accounts.
///
/// Plugins should NOT generally interact with vaults directly, but instead
/// request a controller from the host which manages vault interactions on their
/// behalf. Direct vault interactions are highly secure operations and will
/// generally require increased user permissions.
pub mod vault {
    use alloy::primitives::U256;

    use crate::{
        caip::{AccountId, AssetId},
        entities::VaultId,
    };

    rpc_method!(
        /// Get the balance for all assets in a given account.
        ///
        /// Plugins MAY return zero balances for unsupported assets.
        ///
        /// The list of supported assets MAY change over time.
        vault_get_assets, GetAssets, VaultId, Vec<(AssetId, U256)>
    );

    rpc_method!(
        /// Withdraw an amount of some asset from this vault to another account.
        ///
        /// Vaults MAY reject withdrawals for unsupported assets, insufficient funds,
        /// or for any other reason.
        ///
        /// Vaults MUST reject requests if they cannot fufill them.
        vault_withdraw, Withdraw, (VaultId, AccountId, AssetId, U256), ()
    );

    rpc_method!(
        /// Gets the deposit address for a particular account and asset. Accounts can
        /// also use this to block deposits from unsupported assets or asset classes.
        ///
        /// Plugins MUST return an address if the asset is supported, or an error
        /// if the asset is not supported.
        ///
        /// Because vault implementations are black boxes, any plugin sending an asset
        /// to a vault MUST first call this method to ensure the asset is supported and
        /// the destination address is correct. Destination addresses may change over time,
        /// as might the supported assets.
        vault_get_deposit_address, GetDepositAddress, (VaultId, AssetId), AccountId
    );

    // TODO: Whether this method makes sense. We can't guarantee it will be
    // called on every deposit, so vaults will need to reconcile deposits
    // themselves anyway. It may be better to add callbacks vaults can
    // register for when deposits are made rather than trusting depositors
    // to call this method. rpc_method!(
    //     /// Callback for when an amount is deposited in an account.
    //     ///
    //     /// Acts as a hint to the vault plugin that it should handle the
    // deposit     /// and update its internal state accordingly. The vault
    // cannot assume     /// that this method will always be called for
    // every deposit.     vault_on_deposit, OnDeposit, (VaultId, AccountId,
    // AssetId), () );
}

pub mod coordinator {
    use alloy::primitives::{Address, U256};

    use crate::{
        caip::{AccountId, AssetId, ChainId},
        entities::CoordinatorId,
    };

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct EvmBundle {
        pub inputs: Vec<(AssetId, U256)>,
        // TODO: Consider something like railgun's hasNonDeterministicOutputs flag?
        pub outputs: Vec<AssetId>,
        pub operations: Vec<EvmOperation>,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct EvmOperation {
        pub to: Address,
        pub value: U256,
        pub data: Vec<u8>,
    }

    rpc_method!(
        /// Gets the coordinator to start a new session.
        ///
        /// If Some(accountID) AND the coordinator has previously used
        /// that account for a session, it MUST either resume that session or
        /// return an error.
        ///
        /// If None(accountID) the coordinator may start a new session with any account.
        coordinator_get_session, GetSession, (CoordinatorId, ChainId, Option<AccountId>), AccountId
    );

    rpc_method!(
        /// Get the assets available in the coordinator for a particular account.
        ///
        /// Only valid for accounts that have an active session.
        ///
        /// The coordinator SHOULD only return assets it can guarantee are
        /// available for use in the proposed account.
        coordinator_get_assets, GetAssets, (CoordinatorId, AccountId), Vec<(AssetId, U256)>
    );

    rpc_method!(
        /// Propose a set of EVM operations to be executed by the coordinator from
        /// an account.
        ///
        /// A session MUST have been requested with `RequestSession` prior to calling
        /// this method.
        ///
        /// The coordinator MAY accept or reject the proposal
        ///
        /// After calling this method, the session is considered closed and a new
        /// session MUST be requested for future operations.
        coordinator_propose_evm,
        Propose,
        (CoordinatorId, AccountId, EvmBundle),
        ()
    );
}

pub mod page {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::entities::PageId;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum PageEvent {
        ButtonClicked(String),                          // (button_id)
        FormSubmitted(String, HashMap<String, String>), // (form_id, form_values)
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
