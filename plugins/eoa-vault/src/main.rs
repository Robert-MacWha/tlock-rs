use alloy::{hex, primitives::U256, signers::local::PrivateKeySigner};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::stderr, str::FromStr, sync::Arc};
use tlock_pdk::{
    futures::executor::block_on,
    server::ServerBuilder,
    state::{get_state, set_state},
    tlock_api::{
        RpcMethod,
        caip::{AssetId, ChainId},
        component::{button_input, container, form, heading, submit_input, text, text_input},
        domains::Domain,
        entities::{EntityId, EthProviderId, PageId, VaultId},
        eth, global, host, page, plugin, vault,
    },
    wasmi_pdk::{
        rpc_message::RpcError,
        tracing::{error, info, warn},
        tracing_subscriber::fmt,
        transport::JsonRpcTransport,
    },
};

#[derive(Serialize, Deserialize, Default)]
struct PluginState {
    vaults: HashMap<EntityId, String>,
    eth_provider_id: Option<EthProviderId>,
}

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    info!("Calling Init on Vault Plugin");

    // ? Register the vault's page
    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    let chain_id: ChainId = ChainId::new("eip155".into(), Some("1".into()));
    let provider_id = host::RequestEthProvider
        .call(transport.clone(), chain_id)
        .await?;

    let mut state: PluginState = get_state(transport.clone()).await;
    state.eth_provider_id = provider_id;
    set_state(transport.clone(), &state).await?;

    Ok(())
}

async fn ping(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<String, RpcError> {
    let state: PluginState = get_state(transport.clone()).await;
    let Some(eth_provider_id) = state.eth_provider_id else {
        error!("No Eth provider ID");
        return Err(RpcError::Custom("Missing Eth Provider".into()));
    };

    info!(
        "Pong received, querying chain ID from Eth provider: {}",
        eth_provider_id
    );
    let chain_id = eth::ChainId.call(transport, eth_provider_id).await?;
    Ok(format!("Pong! Connected to chain: {}", chain_id))
}

async fn get_assets(
    transport: Arc<JsonRpcTransport>,
    params: VaultId,
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let vault_id = params;
    info!("Received BalanceOf request for vault: {}", vault_id);

    //? Retrieve the plugin state to get the vault account ID
    let state: PluginState = get_state(transport.clone()).await;

    let vaults = state.vaults;
    let private_key = vaults.get(&vault_id.into()).ok_or_else(|| {
        error!("Vault ID not found in state: {}", vault_id);
        RpcError::InvalidParams
    })?;

    _ = private_key;

    Ok(vec![])
}

async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    info!("OnPageLoad called for page: {}", page_id);

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
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated in Vault Plugin: {:?}", event);

    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "generate_dev_key" => {
            //? Create a vault with a new random private key
            let signer = PrivateKeySigner::random();
            let private_key = signer.to_bytes();
            let private_key_hex = hex::encode(private_key);
            let address = signer.address();

            // Register the vault entity
            let entity_id = host::RegisterEntity
                .call(transport.clone(), Domain::Vault)
                .await?;

            // Save the vault ID and private key in the plugin state
            let mut state: PluginState = get_state(transport.clone()).await;
            state.vaults.insert(entity_id, private_key_hex.clone());
            set_state(transport.clone(), &state).await?;

            let component = container(vec![
                heading("Vault Component"),
                text("New dev private key generated!"),
                text(format!("Your address: {}", address)),
                text(format!("Your private key: {}", private_key_hex)),
            ]);

            host::SetInterface
                .call(transport.clone(), (page_id, component))
                .await?;

            return Ok(());
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "private_key_form" => {
            //? Create a vault from the provided private key
            let Some(private_key) = form_data.get("dev_private_key") else {
                error!("Private key not found in form data");
                return Err(RpcError::Custom("Private key not found in form".into()));
            };

            let Some(private_key) = private_key.first() else {
                error!("Private key value is empty");
                return Err(RpcError::Custom("Private key value is empty".into()));
            };

            info!("Received private key: {}", private_key);

            let signer = PrivateKeySigner::from_str(private_key).map_err(|e| {
                error!("Failed to create signer: {}", e);
                RpcError::Custom("Failed to create signer".into())
            })?;

            let address = signer.address();

            // Register the vault entity
            let entity_id = host::RegisterEntity
                .call(transport.clone(), Domain::Vault)
                .await?;

            // Save the vault ID and private key in the plugin state
            let mut state: PluginState = get_state(transport.clone()).await;
            state.vaults.insert(entity_id, private_key.clone());
            set_state(transport.clone(), &state).await?;

            let component = container(vec![
                heading("Vault Component"),
                text("Private key received!"),
                text(format!("Your address: {}", address)),
                text(format!("Your private key: {}", private_key)),
            ]);

            host::SetInterface
                .call(transport.clone(), (page_id, component))
                .await?;

            return Ok(());
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
        }
    }
    Ok(())
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
