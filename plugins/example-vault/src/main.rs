use alloy::{hex, primitives::U256, signers::local::PrivateKeySigner};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::stderr, str::FromStr, sync::Arc};
use tlock_pdk::{
    async_trait::async_trait,
    dispatcher::{Dispatcher, RpcHandler},
    futures::executor::block_on,
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

struct MyVaultPlugin {
    transport: Arc<JsonRpcTransport>,
}

#[derive(Serialize, Deserialize, Default)]
struct PluginState {
    vaults: HashMap<VaultId, String>, // Maps VaultId to private_key
}

impl MyVaultPlugin {
    pub fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self {
            transport: transport,
        }
    }

    async fn get_state(&self) -> Result<PluginState, RpcErrorCode> {
        let state_bytes = host::GetState
            .call(self.transport.clone(), ())
            .await?
            .ok_or_else(|| {
                error!("State is empty");
                RpcErrorCode::InternalError
            })?;

        let state: PluginState = serde_json::from_slice(&state_bytes).map_err(|e| {
            error!("Failed to deserialize state: {}", e);
            RpcErrorCode::InternalError
        })?;

        Ok(state)
    }

    async fn set_state(&self, state: &PluginState) -> Result<(), RpcErrorCode> {
        let state_bytes = serde_json::to_vec(state).map_err(|e| {
            error!("Failed to serialize state: {}", e);
            RpcErrorCode::InternalError
        })?;

        host::SetState
            .call(self.transport.clone(), state_bytes)
            .await
    }
}

#[async_trait]
impl RpcHandler<plugin::Init> for MyVaultPlugin {
    async fn invoke(&self, _params: ()) -> Result<(), RpcErrorCode> {
        info!("Calling Init on Vault Plugin");

        // ? Register the vault's page
        let page_id = PageId::new("vault_page".to_string());
        host::RegisterEntity
            .call(self.transport.clone(), EntityId::from(page_id))
            .await?;

        Ok(())
    }
}

#[async_trait]
impl RpcHandler<global::Ping> for MyVaultPlugin {
    async fn invoke(&self, _params: ()) -> Result<String, RpcErrorCode> {
        global::Ping.call(self.transport.clone(), ()).await?;
        Ok("pong from vault".to_string())
    }
}

#[async_trait]
impl RpcHandler<vault::GetAssets> for MyVaultPlugin {
    async fn invoke(&self, vault_id: VaultId) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
        info!("Received BalanceOf request for vault: {}", vault_id);

        //? Retrieve the plugin state to get the vault account ID
        let Ok(state) = self.get_state().await else {
            error!("Failed to get plugin state");
            return Err(RpcErrorCode::InternalError);
        };

        let vaults = state.vaults;
        let private_key = vaults.get(&vault_id).ok_or_else(|| {
            error!("Vault ID not found in state: {}", vault_id);
            RpcErrorCode::InvalidParams
        })?;

        // Do some fake work to simulate fetching balances
        let x = fib(32);
        info!("Fake work done, x = {}", x);
        std::hint::black_box(x);

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
}

fn fib(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fib(n - 1) + fib(n - 2),
    }
}

#[async_trait]
impl RpcHandler<page::OnLoad> for MyVaultPlugin {
    async fn invoke(&self, page_id: u32) -> Result<(), RpcErrorCode> {
        info!("OnPageLoad called for page ID: {}", page_id);

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
            .call(self.transport.clone(), (page_id, component))
            .await?;

        Ok(())
    }
}

#[async_trait]
impl RpcHandler<page::OnUpdate> for MyVaultPlugin {
    async fn invoke(&self, (page_id, event): (u32, page::PageEvent)) -> Result<(), RpcErrorCode> {
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
                    .call(self.transport.clone(), entity_id)
                    .await?;

                // Save the vault ID and private key in the plugin state
                let mut state = self.get_state().await.unwrap_or_default();
                state.vaults.insert(vault_id, private_key_hex.clone());
                self.set_state(&state).await?;

                let component = container(vec![
                    heading("Vault Component"),
                    text("New dev private key generated!"),
                    text(&format!("Your address: {}", address)),
                    text(&format!("Your private key: {}", private_key_hex)),
                ]);

                host::SetInterface
                    .call(self.transport.clone(), (page_id, component))
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
                    .call(self.transport.clone(), entity_id)
                    .await?;

                // Save the vault ID and private key in the plugin state
                let state = self.get_state().await.unwrap_or_default();
                let mut state = state;
                state.vaults.insert(vault_id, private_key.clone());
                self.set_state(&state).await?;

                let component = container(vec![
                    heading("Vault Component"),
                    text("Private key received!"),
                    text(&format!("Your address: {}", address)),
                    text(&format!("Your private key: {}", private_key)),
                ]);

                host::SetInterface
                    .call(self.transport.clone(), (page_id, component))
                    .await?;

                return Ok(());
            }
            _ => {
                warn!("Unhandled page event: {:?}", event);
            }
        }
        return Ok(());
    }
}

fn main() {
    fmt()
        .with_writer(stderr)
        // .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(reader, writer);
    let transport = Arc::new(transport);

    let plugin = MyVaultPlugin::new(transport.clone());
    let plugin = Arc::new(plugin);

    let mut dispatcher = Dispatcher::new(plugin);
    dispatcher.register::<global::Ping>();
    dispatcher.register::<plugin::Init>();
    dispatcher.register::<vault::GetAssets>();
    dispatcher.register::<page::OnLoad>();
    dispatcher.register::<page::OnUpdate>();

    let dispatcher = Arc::new(dispatcher);

    block_on(async move {
        let _ = transport.process_next_line(Some(dispatcher)).await;
    });
}
