use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex},
};

use alloy_primitives::U256;
use tlock_hdk::{
    dispatcher::{Dispatcher, RpcHandler},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId},
        entities::{Domain, EntityId, VaultId},
        global, host, plugin,
        vault::{self, BalanceOf},
    },
    wasmi_hdk::plugin::{Plugin, PluginError, PluginId},
    wasmi_pdk::{async_trait::async_trait, rpc_message::RpcErrorCode},
};
use tracing::{info, warn};

pub struct Host {
    plugins: Mutex<HashMap<PluginId, Arc<Plugin>>>,
    entities: Mutex<HashMap<EntityId, PluginId>>,
    domains: Mutex<HashMap<Domain, Vec<EntityId>>>,

    state: Mutex<HashMap<PluginId, Vec<u8>>>,
}

impl Host {
    pub fn new() -> Self {
        Self {
            plugins: Mutex::new(HashMap::new()),
            entities: Mutex::new(HashMap::new()),
            domains: Mutex::new(HashMap::new()),
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Load a plugin from wasm bytes, register it, and return its PluginId
    pub async fn load_plugin(
        self: &Arc<Host>,
        wasm_bytes: &[u8],
        name: &str,
    ) -> Result<PluginId, PluginError> {
        let dispatcher = self.get_dispatcher();

        let mut s = DefaultHasher::new();
        wasm_bytes.hash(&mut s);
        let id = s.finish().to_string().into();
        let plugin = Plugin::new(name, &id, wasm_bytes.to_vec(), dispatcher)
            .map_err(|e| PluginError::SpawnError(e.into()))?;

        self.register_plugin(plugin).await?;
        info!("Loaded plugin '{}' with id {}", name, id);
        Ok(id)
    }

    /// Creates a dispatcher with all Host RPC methods registered
    pub fn get_dispatcher(self: &Arc<Host>) -> Arc<Dispatcher<Host>> {
        let mut dispatcher = Dispatcher::new(Arc::downgrade(&self));
        dispatcher.register::<global::Ping>();
        dispatcher.register::<host::RegisterEntity>();
        dispatcher.register::<host::GetState>();
        dispatcher.register::<host::SetState>();
        dispatcher.register::<vault::BalanceOf>();
        dispatcher.register::<vault::Transfer>();
        dispatcher.register::<vault::GetReceiptAddress>();
        dispatcher.register::<vault::OnReceive>();

        let dispatcher = Arc::new(dispatcher);
        dispatcher
    }

    /// Register a plugin with the host, calling its Init method if it exists
    pub async fn register_plugin(&self, new_plugin: Plugin) -> Result<(), PluginError> {
        let new_plugin = Arc::new(new_plugin);
        self.plugins
            .lock()
            .unwrap()
            .insert(new_plugin.id(), new_plugin.clone());

        info!("Registered plugin {}", new_plugin.id());
        match plugin::Init.call(new_plugin.clone(), ()).await {
            Err(PluginError::RpcError(RpcErrorCode::MethodNotFound))
            | Err(PluginError::RpcError(RpcErrorCode::MethodNotSupported)) => {
                info!(
                    "Plugin {} does not implement Init, skipping",
                    new_plugin.id()
                );
                Ok(())
            }
            Err(e) => {
                warn!("Error calling Init on plugin {}: {:?}", new_plugin.id(), e);
                return Err(e);
            }
            Ok(_) => {
                info!("Plugin {} initialized", new_plugin.id());
                Ok(())
            }
        }
    }

    pub fn get_entities(&self) -> HashMap<EntityId, PluginId> {
        self.entities.lock().unwrap().clone()
    }

    pub fn list_entities(&self) -> Vec<EntityId> {
        let entities = self.entities.lock().unwrap();
        entities.keys().cloned().collect()
    }

    pub fn list_entities_by_domain(&self, domain: &Domain) -> Vec<EntityId> {
        let domains = self.domains.lock().unwrap();
        domains.get(domain).cloned().unwrap_or_default()
    }

    pub fn get_plugin(&self, plugin_id: &PluginId) -> Option<Arc<Plugin>> {
        let plugins = self.plugins.lock().unwrap();
        plugins.get(plugin_id).cloned()
    }

    fn get_plugin_id_for_entity(&self, entity_id: &EntityId) -> Result<PluginId, RpcErrorCode> {
        let entities = self.entities.lock().unwrap();
        match entities.get(entity_id) {
            Some(pid) => Ok(pid.clone()),
            None => {
                warn!("No plugin registered for entity {:?}", entity_id);
                Err(RpcErrorCode::InvalidParams)
            }
        }
    }

    fn get_plugin_for_entity(&self, entity_id: &EntityId) -> Result<Arc<Plugin>, RpcErrorCode> {
        let plugin_id = self.get_plugin_id_for_entity(entity_id)?;
        self.get_plugin(&plugin_id).ok_or_else(|| {
            warn!("Plugin {} not found", plugin_id);
            RpcErrorCode::InvalidParams
        })
    }

    pub async fn ping_plugin(&self, plugin_id: &PluginId) -> Result<String, RpcErrorCode> {
        let plugin = if let Some(plugin) = self.get_plugin(plugin_id) {
            plugin
        } else {
            warn!("Plugin {} not found", plugin_id);
            return Err(RpcErrorCode::InvalidParams);
        };

        let resp = global::Ping.call(plugin, ()).await.map_err(|e| {
            warn!("Error calling Ping on plugin {}: {:?}", plugin_id, e);
            e.as_rpc_code()
        })?;
        Ok(resp)
    }
}

impl Host {
    pub fn ping(&self) -> Result<String, RpcErrorCode> {
        Ok("Pong from host".to_string())
    }

    pub fn register_entity(
        &self,
        plugin_id: &PluginId,
        entity_id: EntityId,
    ) -> Result<(), RpcErrorCode> {
        let mut entities = self.entities.lock().unwrap();
        if let Some(existing_plugin_id) = entities.get(&entity_id) {
            if existing_plugin_id == plugin_id {
                return Ok(());
            } else {
                warn!(
                    "Entity {:?} is already registered by plugin {}",
                    entity_id, existing_plugin_id
                );
                return Err(RpcErrorCode::InvalidParams);
            }
        }

        entities.insert(entity_id.clone(), plugin_id.clone());

        let mut domains = self.domains.lock().unwrap();
        domains
            .entry(entity_id.domain())
            .or_default()
            .push(entity_id.clone());
        Ok(())
    }

    pub async fn get_state(&self, plugin_id: &PluginId) -> Result<Option<Vec<u8>>, RpcErrorCode> {
        Ok(self.state.lock().unwrap().get(plugin_id).cloned())
    }

    pub async fn set_state(
        &self,
        plugin_id: &PluginId,
        state_data: Vec<u8>,
    ) -> Result<(), RpcErrorCode> {
        self.state
            .lock()
            .unwrap()
            .insert(plugin_id.clone(), state_data);
        Ok(())
    }

    pub async fn balance_of(
        &self,
        vault_id: VaultId,
    ) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_plugin_for_entity(&entity_id)?;

        let balance = BalanceOf.call(plugin, vault_id).await.map_err(|e| {
            warn!("Error calling BalanceOf: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(balance)
    }

    pub async fn transfer(
        &self,
        vault_id: VaultId,
        to: AccountId,
        asset: AssetId,
        amount: U256,
    ) -> Result<Result<(), String>, RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_plugin_for_entity(&entity_id)?;

        let result = vault::Transfer
            .call(plugin, (vault_id, to, asset, amount))
            .await
            .map_err(|e| {
                warn!("Error calling Transfer: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(result)
    }

    pub async fn get_receipt_address(
        &self,
        vault_id: VaultId,
        asset: AssetId,
    ) -> Result<Result<AccountId, String>, RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_plugin_for_entity(&entity_id)?;

        let result = vault::GetReceiptAddress
            .call(plugin, (vault_id, asset))
            .await
            .map_err(|e| {
                warn!("Error calling GetReceiptAddress: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(result)
    }

    pub async fn on_receive(&self, vault_id: VaultId, asset: AssetId) -> Result<(), RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_plugin_for_entity(&entity_id)?;

        vault::OnReceive
            .call(plugin, (vault_id, asset))
            .await
            .map_err(|e| {
                warn!("Error calling OnReceive: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(())
    }
}

#[async_trait]
impl RpcHandler<global::Ping> for Host {
    async fn invoke(&self, plugin_id: PluginId, _params: ()) -> Result<String, RpcErrorCode> {
        info!("Plugin {} sent ping", plugin_id);
        self.ping()
    }
}

#[async_trait]
impl RpcHandler<host::RegisterEntity> for Host {
    async fn invoke(&self, plugin_id: PluginId, entity_id: EntityId) -> Result<(), RpcErrorCode> {
        info!(
            "Plugin {} requested registration of entity {:?}",
            plugin_id, entity_id
        );
        self.register_entity(&plugin_id, entity_id)
    }
}

#[async_trait]
impl RpcHandler<host::GetState> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        _params: (),
    ) -> Result<Option<Vec<u8>>, RpcErrorCode> {
        info!("Plugin {} requested its state", plugin_id);
        self.get_state(&plugin_id).await
    }
}

#[async_trait]
impl RpcHandler<host::SetState> for Host {
    async fn invoke(&self, plugin_id: PluginId, state_data: Vec<u8>) -> Result<(), RpcErrorCode> {
        info!("Plugin {} requested to set its state", plugin_id);
        self.set_state(&plugin_id, state_data).await
    }
}

#[async_trait]
impl RpcHandler<vault::BalanceOf> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        vault_id: VaultId,
    ) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
        info!("Plugin {} requested balance of {:?}", plugin_id, vault_id);
        self.balance_of(vault_id).await
    }
}

#[async_trait]
impl RpcHandler<vault::Transfer> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: (VaultId, AccountId, AssetId, U256),
    ) -> Result<Result<(), String>, RpcErrorCode> {
        let (vault, to, asset, amount) = params;
        info!(
            "Plugin {} requested transfer of {} {:?} from vault {:?} to {:?}",
            plugin_id, amount, asset, vault, to
        );

        self.transfer(vault, to, asset, amount).await
    }
}

#[async_trait]
impl RpcHandler<vault::GetReceiptAddress> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: (VaultId, AssetId),
    ) -> Result<Result<AccountId, String>, RpcErrorCode> {
        let (vault_id, asset) = params;
        info!(
            "Plugin {} requested receipt address for asset {:?} in vault {:?}",
            plugin_id, asset, vault_id
        );

        self.get_receipt_address(vault_id, asset).await
    }
}

#[async_trait]
impl RpcHandler<vault::OnReceive> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: (VaultId, AssetId),
    ) -> Result<(), RpcErrorCode> {
        let (vault_id, asset) = params;
        info!(
            "Plugin {} notified of receipt of asset {:?} in vault {:?}",
            plugin_id, asset, vault_id
        );

        self.on_receive(vault_id, asset).await
    }
}
