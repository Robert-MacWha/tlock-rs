use std::{collections::HashMap, io::stderr, sync::Arc};

use serde::{Deserialize, Serialize};
use tlock_pdk::{
    server::PluginServer,
    state::{get_state, set_state},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId},
        component::{
            Component, button_input, container, dropdown, form, heading, heading2, submit_input,
            text, text_input, unordered_list,
        },
        domains::Domain,
        entities::{PageId, VaultId},
        global, host, page, plugin,
        vault::{self},
    },
    wasmi_plugin_pdk::{rpc_message::RpcError, transport::JsonRpcTransport},
};
use tracing::{info, warn};
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize, Default, Debug)]
struct PluginState {
    page_id: Option<PageId>,
    vault_id: Option<VaultId>,
    cached_assets: Vec<(AssetId, alloy::primitives::U256)>,
    last_message: Option<String>,
}

// ---------- Plugin Handlers ----------

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    info!("Initializing Vault Page Plugin");

    // Register the vault page
    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    handle_request_vault(&transport).await?;

    Ok(())
}

async fn ping(_: Arc<JsonRpcTransport>, _: ()) -> Result<String, RpcError> {
    Ok("pong".to_string())
}

// ---------- UI Handlers ----------

async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    info!("Page loaded: {}", page_id);

    let mut state: PluginState = get_state(transport.clone()).await;
    state.page_id = Some(page_id);
    set_state(transport.clone(), &state).await?;

    let component = build_ui(&state);
    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated: {:?}", event);

    let mut state: PluginState = get_state(transport.clone()).await;

    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "refresh_assets" => {
            handle_refresh_assets(&transport, &mut state).await?;
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "get_deposit_form" => {
            handle_get_deposit(&transport, &mut state, form_data).await?;
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "withdraw_form" => {
            handle_withdraw(&transport, &mut state, form_data).await?;
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    }

    set_state(transport.clone(), &state).await?;

    let component = build_ui(&state);
    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

// ---------- Event Handlers ----------

async fn handle_request_vault(transport: &Arc<JsonRpcTransport>) -> Result<(), RpcError> {
    info!("Requesting vault from host");

    let vault_id = host::RequestVault.call(transport.clone(), ()).await?;
    let mut state: PluginState = get_state(transport.clone()).await;
    state.vault_id = Some(vault_id);
    state.last_message = Some(format!("Vault selected: {}", vault_id));
    info!("Vault selected: {}", vault_id);

    set_state(transport.clone(), &state).await?;
    host::SetPage
        .call(
            transport.clone(),
            (state.page_id.unwrap(), build_ui(&state)),
        )
        .await?;

    Ok(())
}

async fn handle_refresh_assets(
    transport: &Arc<JsonRpcTransport>,
    state: &mut PluginState,
) -> Result<(), RpcError> {
    let Some(vault_id) = state.vault_id else {
        state.last_message = Some("No vault selected".to_string());
        return Ok(());
    };

    info!("Refreshing assets for vault: {}", vault_id);

    let assets = vault::GetAssets
        .call(transport.clone(), vault_id)
        .await
        .map_err(|e| {
            warn!("Error fetching assets: {:?}", e);
            e
        })?;

    state.cached_assets = assets;
    state.last_message = Some(format!("Fetched {} assets", state.cached_assets.len()));

    Ok(())
}

async fn handle_get_deposit(
    transport: &Arc<JsonRpcTransport>,
    state: &mut PluginState,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let Some(vault_id) = state.vault_id else {
        state.last_message = Some("No vault selected".to_string());
        return Ok(());
    };

    let Some(asset_str) = form_data.get("asset") else {
        state.last_message = Some("No asset selected".to_string());
        return Ok(());
    };

    let asset_id: AssetId = asset_str
        .parse()
        .map_err(|e| RpcError::Custom(format!("Invalid asset ID: {}", e)))?;

    info!("Getting deposit address for asset: {}", asset_id);

    let account_id = vault::GetDepositAddress
        .call(transport.clone(), (vault_id, asset_id.clone()))
        .await
        .map_err(|e| {
            warn!("Error getting deposit address: {:?}", e);
            e
        })?;

    state.last_message = Some(format!("Deposit address for {}: {}", asset_id, account_id));

    Ok(())
}

async fn handle_withdraw(
    transport: &Arc<JsonRpcTransport>,
    state: &mut PluginState,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let Some(vault_id) = state.vault_id else {
        state.last_message = Some("No vault selected".to_string());
        return Ok(());
    };

    let Some(to_address_str) = form_data.get("to_address") else {
        state.last_message = Some("Missing to_address".to_string());
        return Ok(());
    };

    let Some(asset_str) = form_data.get("asset") else {
        state.last_message = Some("Missing asset".to_string());
        return Ok(());
    };

    let Some(amount_str) = form_data.get("amount") else {
        state.last_message = Some("Missing amount".to_string());
        return Ok(());
    };

    let to_address: AccountId = to_address_str
        .parse()
        .map_err(|e| RpcError::Custom(format!("Invalid to_address: {}", e)))?;

    let asset_id: AssetId = asset_str
        .parse()
        .map_err(|e| RpcError::Custom(format!("Invalid asset ID: {}", e)))?;

    let amount: alloy::primitives::U256 = amount_str
        .parse()
        .map_err(|_| RpcError::Custom("Invalid amount".to_string()))?;

    info!(
        "Withdrawing {} {} from vault {} to {}",
        amount, asset_id, vault_id, to_address
    );

    vault::Withdraw
        .call(transport.clone(), (vault_id, to_address, asset_id, amount))
        .await
        .map_err(|e| {
            warn!("Error withdrawing: {:?}", e);
            e
        })?;

    state.last_message = Some("Withdrawal successful".to_string());

    Ok(())
}

// ---------- UI Builders ----------

fn build_ui(state: &PluginState) -> Component {
    let mut sections = vec![
        heading("Vault Page"),
        text("A simple UI for interacting with vault plugins"),
    ];

    // Status section
    let Some(vault_id) = &state.vault_id else {
        sections.push(text("No vault selected"));
        return container(sections);
    };

    sections.extend(vec![
        text(format!("Current vault: {}", vault_id)),
        text(format!(
            "Status: {}",
            state.last_message.as_deref().unwrap_or("OK")
        )),
        heading2("Assets"),
        button_input("refresh_assets", "Refresh Assets"),
    ]);

    // Assets section
    if state.cached_assets.is_empty() {
        sections.push(text("No assets. Click 'Refresh Assets' to load."));
        return container(sections);
    }

    let balances = state
        .cached_assets
        .iter()
        .map(|(id, bal)| (id.to_string(), text(format!("{}: {}", id, bal))));

    let asset_options = state.cached_assets.iter().map(|(id, _)| id.to_string());

    sections.extend(vec![
        // Assets list
        unordered_list(balances),
        // Deposit section
        heading2("Get Deposit Address"),
        form(
            "get_deposit_form",
            vec![
                dropdown("asset", asset_options.clone(), None),
                submit_input("Get Address"),
            ],
        ),
        // Withdraw section
        heading2("Withdraw"),
        form(
            "withdraw_form",
            vec![
                text_input("to_address", "Recipient address (CAIP-10)"),
                dropdown("asset", asset_options, None),
                text_input("amount", "Amount (wei)"),
                submit_input("Withdraw"),
            ],
        ),
    ]);

    container(sections)
}

// ---------- Entrypoint ----------

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting Vault Page Plugin...");

    PluginServer::new_with_transport()
        .with_method(plugin::Init, init)
        .with_method(global::Ping, ping)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .run();
}
