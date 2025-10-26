use alloy::{hex, primitives::U256, signers::local::PrivateKeySigner};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::stderr, str::FromStr, sync::Arc};
use tlock_pdk::{
    futures::executor::block_on,
    server::ServerBuilder,
    state::{get_state, get_state_or_default, set_state},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId},
        component::{button_input, container, form, heading, submit_input, text, text_input},
        entities::{EntityId, PageId, VaultId},
        global, host, page, plugin, vault,
    },
    wasmi_pdk::{
        rpc_message::RpcErrorCode,
        tracing::{error, info, warn},
        tracing_subscriber::fmt,
        transport::JsonRpcTransport,
    },
};

#[derive(Serialize, Deserialize, Default)]
struct PluginState {
    vaults: HashMap<VaultId, String>, // Maps VaultId to private_key
}

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcErrorCode> {
    info!("Calling Init on Vault Plugin");

    // ? Register the vault's page
    let page_id = PageId::new("vault_page".to_string());
    host::RegisterEntity
        .call(transport.clone(), EntityId::from(page_id))
        .await?;

    Ok(())
}

async fn ping(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<String, RpcErrorCode> {
    global::Ping.call(transport.clone(), ()).await?;
    Ok("pong from vault".to_string())
}

async fn get_assets(
    transport: Arc<JsonRpcTransport>,
    params: VaultId,
) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
    let vault_id = params;
    info!("Received BalanceOf request for vault: {}", vault_id);

    //? Retrieve the plugin state to get the vault account ID
    let state: PluginState = get_state(transport.clone()).await?;

    let vaults = state.vaults;
    let private_key = vaults.get(&vault_id).ok_or_else(|| {
        error!("Vault ID not found in state: {}", vault_id);
        RpcErrorCode::InvalidParams
    })?;

    //? Here you would normally query the balances from an external source.
    //? For this example, we'll return a dummy balance.
    info!("Fetching balances for account: {:?}", private_key);
    let dummy_asset_id = AssetId::new(
        1,
        "erc20".into(),
        "0x11223344556677889900aabbccddeeff".into(),
    );

    Ok(vec![(dummy_asset_id, U256::from(1000u64))])
}

async fn on_load(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, u32),
) -> Result<(), RpcErrorCode> {
    let (_page_id, interface_id) = params;
    info!("OnPageLoad called for interface ID: {}", interface_id);

    let component = container(vec![
        heading("Vault Component"),
        text("This is an example vault plugin. Please enter a dev private key."),
        button_input("generate_dev_key", "Generate Dev Key"),
        form(
            "private_key_form",
            vec![
                text_input("dev_private_key", "Enter your dev private key"),
                submit_input("Submit"),
            ],
        ),
    ]);

    host::SetInterface
        .call(transport.clone(), (interface_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, u32, page::PageEvent),
) -> Result<(), RpcErrorCode> {
    let (_page_id, interface_id, event) = params;
    info!("Page updated in Vault Plugin: {:?}", event);

    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "generate_dev_key" => {
            //? Create a vault with a new random private key
            let signer = PrivateKeySigner::random();
            let private_key = signer.to_bytes();
            let private_key_hex = hex::encode(private_key);
            let address = signer.address();

            // Register the vault entity
            let account_id = AccountId::new(1, address);
            let vault_id = VaultId::new(account_id.to_string());
            let entity_id = vault_id.as_entity_id();

            host::RegisterEntity
                .call(transport.clone(), entity_id)
                .await?;

            // Save the vault ID and private key in the plugin state
            let mut state: PluginState = get_state_or_default(transport.clone()).await;
            state.vaults.insert(vault_id, private_key_hex.clone());
            set_state(transport.clone(), &state).await?;

            let component = container(vec![
                heading("Vault Component"),
                text("New dev private key generated!"),
                text(&format!("Your address: {}", address)),
                text(&format!("Your private key: {}", private_key_hex)),
            ]);

            host::SetInterface
                .call(transport.clone(), (interface_id, component))
                .await?;

            return Ok(());
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "private_key_form" => {
            //? Create a vault from the provided private key
            let Some(private_key) = form_data.get("dev_private_key") else {
                error!("Private key not found in form data");
                return Err(RpcErrorCode::InvalidParams);
            };

            let Some(private_key) = private_key.get(0) else {
                error!("Private key value is empty");
                return Err(RpcErrorCode::InvalidParams);
            };

            info!("Received private key: {}", private_key);

            let signer = PrivateKeySigner::from_str(private_key).map_err(|e| {
                error!("Failed to create signer: {}", e);
                RpcErrorCode::InvalidParams
            })?;

            let address = signer.address();

            // Register the vault entity
            let account_id = AccountId::new(1, address);
            let vault_id = VaultId::new(account_id.to_string());
            let entity_id = vault_id.as_entity_id();

            host::RegisterEntity
                .call(transport.clone(), entity_id)
                .await?;

            // Save the vault ID and private key in the plugin state
            let mut state: PluginState = get_state_or_default(transport.clone()).await;
            state.vaults.insert(vault_id, private_key.clone());
            set_state(transport.clone(), &state).await?;

            let component = container(vec![
                heading("Vault Component"),
                text("Private key received!"),
                text(&format!("Your address: {}", address)),
                text(&format!("Your private key: {}", private_key)),
            ]);

            host::SetInterface
                .call(transport.clone(), (interface_id, component))
                .await?;

            return Ok(());
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
        }
    }
    return Ok(());
}

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(reader, writer);
    let transport = Arc::new(transport);

    let plugin = ServerBuilder::new(transport.clone())
        .with_method(plugin::Init, init)
        .with_method(global::Ping, ping)
        .with_method(vault::GetAssets, get_assets)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .finish();
    let plugin = Arc::new(plugin);

    block_on(async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    });
}
