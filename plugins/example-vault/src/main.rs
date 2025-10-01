use log::info;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tlock_pdk::{
    async_trait::async_trait,
    dispatcher::{Dispatcher, RpcHandler},
    futures::executor::block_on,
    tlock_api::{
        RpcMethod,
        alloy_primitives::{U256, address},
        caip::{AccountId, AssetId},
        entities::{EntityId, VaultId},
        global, host, plugin, vault,
    },
    wasmi_pdk::{rpc_message::RpcErrorCode, transport::JsonRpcTransport},
};

struct MyVaultPlugin {
    transport: Arc<JsonRpcTransport>,
}

#[derive(Serialize, Deserialize)]
struct PluginState {
    vaults: HashMap<VaultId, AccountId>,
}

impl MyVaultPlugin {
    pub fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self {
            transport: transport,
        }
    }
}

#[async_trait]
impl RpcHandler<plugin::Init> for MyVaultPlugin {
    async fn invoke(&self, _params: ()) -> Result<(), RpcErrorCode> {
        log::info!("Calling Init on Vault Plugin");

        //? Create a new vault entity and register it
        let account_id = AccountId::new(1, address!("0x0102030405060708090a0b0c0d0e0f1011121314"));
        let vault_id = VaultId::new(account_id.to_string());
        let entity_id = EntityId::Vault(vault_id.clone());
        host::RegisterEntity
            .call(self.transport.clone(), entity_id)
            .await?;

        //? Save the vault account ID in the plugin state for future reference
        let mut state = PluginState {
            vaults: HashMap::new(),
        };
        state.vaults.insert(vault_id, account_id);

        let state = serde_json::to_vec(&state).map_err(|e| {
            log::error!("Failed to serialize state: {}", e);
            RpcErrorCode::InternalError
        })?;
        host::SetState.call(self.transport.clone(), state).await?;

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
impl RpcHandler<vault::BalanceOf> for MyVaultPlugin {
    async fn invoke(&self, vault_id: VaultId) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
        info!("Received BalanceOf request for vault: {}", vault_id);

        //? Retrieve the plugin state to get the vault account ID
        let state_bytes = host::GetState
            .call(self.transport.clone(), ())
            .await?
            .ok_or_else(|| {
                log::error!("`BalanceOf` called before `Init`");
                RpcErrorCode::InternalError
            })?;

        info!("State bytes length: {}", state_bytes.len());

        let state: PluginState = serde_json::from_slice(&state_bytes).map_err(|e| {
            log::error!("Failed to deserialize state: {}", e);
            RpcErrorCode::InternalError
        })?;

        info!("Deserialized state: {:?}", state.vaults);

        let vaults = state.vaults;
        let account_id = vaults.get(&vault_id).ok_or_else(|| {
            log::error!("Vault ID not found in state: {}", vault_id);
            RpcErrorCode::InvalidParams
        })?;

        //? Here you would normally query the balances from an external source.
        //? For this example, we'll return a dummy balance.
        log::info!("Fetching balances for account: {:?}", account_id);
        let dummy_asset_id = AssetId::new(
            1,
            "erc20".into(),
            "0x11223344556677889900aabbccddeeff".into(),
        );

        Ok(vec![(dummy_asset_id, U256::from(1000u64))])
    }
}

fn main() {
    stderrlog::new()
        .verbosity(stderrlog::LogLevelNum::Trace)
        .init()
        .unwrap();
    log::trace!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let transport = Arc::new(transport);

    let plugin = MyVaultPlugin::new(transport.clone());
    let plugin = Arc::new(plugin);

    let mut dispatcher = Dispatcher::new(plugin);
    dispatcher.register::<global::Ping>();
    dispatcher.register::<plugin::Init>();
    dispatcher.register::<vault::BalanceOf>();

    let dispatcher = Arc::new(dispatcher);

    block_on(async move {
        let _ = transport.process_next_line(Some(dispatcher)).await;
    });
}
