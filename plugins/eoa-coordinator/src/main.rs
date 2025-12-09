//! Basic EOA Coordinator Plugin implementation.
//!
//! NOT DESIGNED FOR PRODUCTION USE.
//!
//! This is a minimal, very insecure implementation of a Coordinator Plugin. It
//! stores its private key in memory and in plaintext host storage, does not
//! authenticate requests, and does not do any validation of incoming data. It
//! is intended purely for demonstration and testing.
use std::{io::stderr, sync::Arc};

use alloy::{hex, primitives::FixedBytes, signers::local::PrivateKeySigner};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    server::PluginServer,
    state::{set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        alloy::primitives::U256,
        caip::{AccountId, AssetId, ChainId},
        component::{button_input, container, form, heading, submit_input, text, text_input},
        coordinator,
        domains::Domain,
        entities::{CoordinatorId, EntityId, PageId, VaultId},
        global, host,
        page::{self, PageEvent},
        plugin, vault,
    },
    wasmi_plugin_pdk::{rpc_message::RpcError, transport::JsonRpcTransport},
};
use tracing::info;
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize, Debug)]
struct State {
    vault_id: VaultId,
    coordinator_id: Option<EntityId>,
    private_key: Option<FixedBytes<32>>,
    account: Option<AccountId>,
}

const CHAIN_ID: u64 = 11155111; // Sepolia

async fn ping(transport: Arc<JsonRpcTransport>, _: ()) -> Result<String, RpcError> {
    global::Ping.call(transport, ()).await?;
    Ok("pong".to_string())
}

async fn init(transport: Arc<JsonRpcTransport>, _: ()) -> Result<(), RpcError> {
    let vault_id = host::RequestVault
        .call(transport.clone(), ())
        .await?
        .ok_or(RpcError::Custom("Expected vault ID".into()))?;
    info!("Obtained vault ID: {:?}", vault_id);

    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    let state = State {
        vault_id,
        coordinator_id: None,
        private_key: None,
        account: None,
    };

    set_state(transport.clone(), &state).await?;
    Ok(())
}

async fn get_session(
    transport: Arc<JsonRpcTransport>,
    params: (CoordinatorId, ChainId, Option<AccountId>),
) -> Result<AccountId, RpcError> {
    let state: State = try_get_state(transport.clone()).await?;
    let (coordinator_id, chain_id, maybe_account_id) = params;

    if Some(coordinator_id.into()) != state.coordinator_id {
        return Err(RpcError::Custom("Invalid CoordinatorId".into()));
    }

    // TODO: Support arbitrary evm chain IDs
    if chain_id != ChainId::new_evm(CHAIN_ID) {
        return Err(RpcError::Custom("Invalid ChainId".into()));
    }

    let Some(state_account_id) = state.account else {
        return Err(RpcError::Custom("No Account configured".into()));
    };

    if let Some(account_id) = maybe_account_id
        && account_id != state_account_id
    {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    Ok(state_account_id)
}

async fn get_assets(
    transport: Arc<JsonRpcTransport>,
    params: (CoordinatorId, AccountId),
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let state: State = try_get_state(transport.clone()).await?;
    let (coordinator_id, account_id) = params;

    if Some(coordinator_id.into()) != state.coordinator_id {
        return Err(RpcError::Custom("Invalid CoordinatorId".into()));
    }

    let Some(state_account_id) = state.account else {
        return Err(RpcError::Custom("No Account configured".into()));
    };

    if account_id != state_account_id {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    // TODO: Filter assets by those on the same chain as the account
    vault::GetAssets
        .call(transport.clone(), state.vault_id)
        .await
}

async fn propose(
    transport: Arc<JsonRpcTransport>,
    params: (CoordinatorId, AccountId, coordinator::EvmBundle),
) -> Result<(), RpcError> {
    let state: State = try_get_state(transport.clone()).await?;
    let (coordinator_id, account_id, bundle) = params;

    if Some(coordinator_id.into()) != state.coordinator_id {
        return Err(RpcError::Custom("Invalid CoordinatorId".into()));
    }

    let Some(state_account_id) = state.account else {
        return Err(RpcError::Custom("No Account configured".into()));
    };

    if account_id != state_account_id {
        return Err(RpcError::Custom("Invalid AccountId".into()));
    }

    // Verify that the vault has the necessary assets to cover the bundle.
    let vault_assets = vault::GetAssets
        .call(transport.clone(), state.vault_id)
        .await?;

    for (asset_id, amount) in &bundle.inputs {
        let vault_amount = vault_assets
            .iter()
            .find_map(|(id, amt)| (id == asset_id).then_some(*amt))
            .unwrap_or(U256::ZERO);

        if &vault_amount < amount {
            return Err(RpcError::Custom(format!(
                "Insufficient asset {asset_id:?} in vault"
            )));
        }
    }

    // Transfer requested assets from the vault to the controller's account
    for (asset_id, amount) in &bundle.inputs {
        info!("Transferring asset {:?} amount {:?}", asset_id, amount);
    }

    // Perform proposed operations
    for operation in bundle.operations {
        info!("Proposed operation: {:?}", operation);
    }

    // Return any assets in the controller's account back to the vault
    for (asset_id, _) in &bundle.inputs {
        info!("Returning input asset {:?} to vault", asset_id);
    }

    for asset_id in &bundle.outputs {
        info!("Returning output asset {:?} to vault", asset_id);
    }

    Ok(())
}

// ---------- UI Handlers ----------
async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    let component = container(vec![
        heading("EOA Coordinator"),
        text("This is an example dev coordinator plugin."),
        form(
            "private_key_form",
            vec![
                text_input("dev_private_key", "Enter your private key"),
                submit_input("Update"),
            ],
        ),
        button_input("generate_dev_key", "Generate Dev Key"),
    ]);

    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    props: (PageId, PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = props;

    let private_key_hex = match event {
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "private_key_form" => {
            let Some(pk) = form_data.get("dev_private_key") else {
                return Err(RpcError::Custom("Private key not found in form".into()));
            };
            pk.clone()
        }
        page::PageEvent::ButtonClicked(button_id) if button_id == "generate_dev_key" => {
            let signer = PrivateKeySigner::random();
            let private_key = signer.to_bytes();
            hex::encode(private_key)
        }
        _ => {
            return Ok(());
        }
    };

    let signer: PrivateKeySigner = private_key_hex
        .parse()
        .map_err(|_| RpcError::Custom("Invalid private key".into()))?;
    let address = signer.address();
    let account_id = AccountId::new_evm(CHAIN_ID, address);

    let coordinator_id = host::RegisterEntity
        .call(transport.clone(), Domain::Coordinator)
        .await?;

    let mut state: State = try_get_state(transport.clone()).await?;
    state.coordinator_id = Some(coordinator_id);
    state.private_key = Some(signer.to_bytes());
    state.account = Some(account_id.clone());
    set_state(transport.clone(), &state).await?;

    let component = text(format!(
        "Coordinator created!\n\nAddress: {}\n\nPrivate Key: {}",
        address, private_key_hex
    ));
    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    todo!()
}

fn main() {
    fmt().with_writer(stderr).init();
    info!("Starting plugin...");

    PluginServer::new_with_transport()
        .with_method(global::Ping, ping)
        .with_method(plugin::Init, init)
        .with_method(coordinator::GetSession, get_session)
        .with_method(coordinator::GetAssets, get_assets)
        .with_method(coordinator::Propose, propose)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .run();
}
