use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex},
};

use alloy::primitives::U256;
use tlock_hdk::{
    dispatcher::{Dispatcher, RpcHandler},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId},
        component::Component,
        domains::Domain,
        entities::{EntityId, VaultId},
        eth, global, host, page, plugin,
        vault::{self},
    },
    wasmi_hdk::plugin::{Plugin, PluginError, PluginId},
    wasmi_pdk::{async_trait::async_trait, rpc_message::RpcErrorCode},
};
use tracing::{info, warn};

pub struct Host {
    plugins: Mutex<HashMap<PluginId, Arc<Plugin>>>,
    entities: Mutex<HashMap<EntityId, PluginId>>,
    domains: Mutex<HashMap<Domain, Vec<EntityId>>>,

    // TODO: Restrict these to a max size / otherwise prevent plugins from abusing storage
    state: Mutex<HashMap<PluginId, Vec<u8>>>,
    interfaces: Mutex<HashMap<u32, Component>>, // interface_id -> Component
}

impl Host {
    pub fn new() -> Self {
        Self {
            plugins: Mutex::new(HashMap::new()),
            entities: Mutex::new(HashMap::new()),
            domains: Mutex::new(HashMap::new()),
            state: Mutex::new(HashMap::new()),
            interfaces: Mutex::new(HashMap::new()),
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
        dispatcher.register::<host::SetInterface>();
        dispatcher.register::<vault::GetAssets>();
        dispatcher.register::<vault::Withdraw>();
        dispatcher.register::<vault::GetDepositAddress>();
        dispatcher.register::<vault::OnDeposit>();
        dispatcher.register::<page::OnLoad>();
        dispatcher.register::<page::OnUpdate>();
        dispatcher.register::<eth::BlockNumber>();
        dispatcher.register::<eth::Call>();
        dispatcher.register::<eth::GetBalance>();

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

    /// Get all registered entities
    pub fn get_entities(&self) -> Vec<EntityId> {
        let entities = self.entities.lock().unwrap();
        entities.keys().cloned().collect()
    }

    /// Get all registered plugins
    pub fn get_plugins(&self) -> Vec<PluginId> {
        let plugins = self.plugins.lock().unwrap();
        plugins.keys().cloned().collect()
    }

    /// Get all entities for a given domain
    pub fn get_entities_by_domain(&self, domain: &Domain) -> Vec<EntityId> {
        let domains = self.domains.lock().unwrap();
        domains.get(domain).cloned().unwrap_or_default()
    }

    /// Get a plugin by its PluginId
    pub fn get_plugin(&self, plugin_id: &PluginId) -> Option<Arc<Plugin>> {
        let plugins = self.plugins.lock().unwrap();
        plugins.get(plugin_id).cloned()
    }

    /// Get the PluginId responsible for a given EntityId
    pub fn get_entity_plugin_id(&self, entity_id: &EntityId) -> Option<PluginId> {
        let entities = self.entities.lock().unwrap();
        entities.get(entity_id).cloned()
    }

    pub fn get_entity_plugin(&self, entity_id: &EntityId) -> Option<Arc<Plugin>> {
        let plugin_id = self.get_entity_plugin_id(entity_id)?;
        self.get_plugin(&plugin_id)
    }

    pub fn get_interfaces(&self) -> HashMap<u32, Component> {
        let interfaces = self.interfaces.lock().unwrap();
        interfaces.clone()
    }

    pub fn get_interface(&self, interface_id: u32) -> Option<Component> {
        let interfaces = self.interfaces.lock().unwrap();
        interfaces.get(&interface_id).cloned()
    }

    ///? Helper to get the plugin or return an RpcErrorCode if not found
    fn get_entity_plugin_error(&self, entity_id: &EntityId) -> Result<Arc<Plugin>, RpcErrorCode> {
        let plugin = self.get_entity_plugin(entity_id).ok_or_else(|| {
            warn!("Entity {:?} not found", entity_id);
            RpcErrorCode::InvalidParams
        })?;
        Ok(plugin)
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

// TODO: I should use a macro to reduce boilerplate, and also add permission checks + proper logging.

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

    pub async fn set_interface(
        &self,
        plugin_id: &PluginId,
        interface_id: u32,
        component: Component,
    ) -> Result<(), RpcErrorCode> {
        info!(
            "Plugin {} requested set interface {}: {:?}",
            plugin_id, interface_id, component
        );

        self.interfaces
            .lock()
            .unwrap()
            .insert(interface_id, component);
        Ok(())
    }

    pub async fn vault_get_assets(
        &self,
        vault_id: VaultId,
    ) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_entity_plugin_error(&entity_id)?;

        let balance = vault::GetAssets.call(plugin, vault_id).await.map_err(|e| {
            warn!("Error calling BalanceOf: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(balance)
    }

    pub async fn vault_withdraw(
        &self,
        vault_id: VaultId,
        to: AccountId,
        asset: AssetId,
        amount: U256,
    ) -> Result<Result<(), String>, RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_entity_plugin_error(&entity_id)?;

        let result = vault::Withdraw
            .call(plugin, (vault_id, to, asset, amount))
            .await
            .map_err(|e| {
                warn!("Error calling Transfer: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(result)
    }

    pub async fn vault_get_deposit_address(
        &self,
        vault_id: VaultId,
        asset: AssetId,
    ) -> Result<Result<AccountId, String>, RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_entity_plugin_error(&entity_id)?;

        let result = vault::GetDepositAddress
            .call(plugin, (vault_id, asset))
            .await
            .map_err(|e| {
                warn!("Error calling GetReceiptAddress: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(result)
    }

    pub async fn vault_on_deposit(
        &self,
        vault_id: VaultId,
        asset: AssetId,
    ) -> Result<(), RpcErrorCode> {
        let entity_id = vault_id.as_entity_id();
        let plugin = self.get_entity_plugin_error(&entity_id)?;

        vault::OnDeposit
            .call(plugin, (vault_id, asset))
            .await
            .map_err(|e| {
                warn!("Error calling OnReceive: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(())
    }

    pub async fn page_on_load(
        &self,
        plugin_id: &PluginId,
        interface_id: u32,
    ) -> Result<(), RpcErrorCode> {
        let plugin = if let Some(plugin) = self.get_plugin(plugin_id) {
            plugin
        } else {
            warn!("Plugin {} not found", plugin_id);
            return Err(RpcErrorCode::InvalidParams);
        };

        page::OnLoad.call(plugin, interface_id).await.map_err(|e| {
            warn!("Error calling OnPageLoad on plugin {}: {:?}", plugin_id, e);
            e.as_rpc_code()
        })?;
        Ok(())
    }

    pub async fn page_on_update(
        &self,
        plugin_id: &PluginId,
        interface_id: u32,
        event: page::PageEvent,
    ) -> Result<(), RpcErrorCode> {
        let plugin = if let Some(plugin) = self.get_plugin(plugin_id) {
            plugin
        } else {
            warn!("Plugin {} not found", plugin_id);
            return Err(RpcErrorCode::InvalidParams);
        };

        page::OnUpdate
            .call(plugin, (interface_id, event))
            .await
            .map_err(|e| {
                warn!(
                    "Error calling OnPageUpdate on plugin {}: {:?}",
                    plugin_id, e
                );
                e.as_rpc_code()
            })?;
        Ok(())
    }

    pub async fn eth_provider_block_number(
        &self,
        plugin_id: &PluginId,
    ) -> Result<u64, RpcErrorCode> {
        let plugin = if let Some(plugin) = self.get_plugin(plugin_id) {
            plugin
        } else {
            warn!("Plugin {} not found", plugin_id);
            return Err(RpcErrorCode::InvalidParams);
        };

        let block_number = eth::BlockNumber.call(plugin, ()).await.map_err(|e| {
            warn!("Error calling BlockNumber on plugin {}: {:?}", plugin_id, e);
            e.as_rpc_code()
        })?;
        Ok(block_number)
    }

    pub async fn eth_provider_call(
        &self,
        plugin_id: &PluginId,
        params: <eth::Call as RpcMethod>::Params,
    ) -> Result<<eth::Call as RpcMethod>::Output, RpcErrorCode> {
        let plugin = if let Some(plugin) = self.get_plugin(plugin_id) {
            plugin
        } else {
            warn!("Plugin {} not found", plugin_id);
            return Err(RpcErrorCode::InvalidParams);
        };

        let resp = eth::Call.call(plugin, params).await.map_err(|e| {
            warn!("Error calling Call on plugin {}: {:?}", plugin_id, e);
            e.as_rpc_code()
        })?;
        Ok(resp)
    }

    pub async fn eth_provider_get_balance(
        &self,
        plugin_id: &PluginId,
        params: <eth::GetBalance as RpcMethod>::Params,
    ) -> Result<<eth::GetBalance as RpcMethod>::Output, RpcErrorCode> {
        let plugin = if let Some(plugin) = self.get_plugin(plugin_id) {
            plugin
        } else {
            warn!("Plugin {} not found", plugin_id);
            return Err(RpcErrorCode::InvalidParams);
        };

        let resp = eth::GetBalance.call(plugin, params).await.map_err(|e| {
            warn!("Error calling GetBalance on plugin {}: {:?}", plugin_id, e);
            e.as_rpc_code()
        })?;
        Ok(resp)
    }
}

// TODO: I can totally use a macro for all this boilerplate

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
impl RpcHandler<host::SetInterface> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: (u32, Component),
    ) -> Result<(), RpcErrorCode> {
        let (interface_id, component) = params;
        self.set_interface(&plugin_id, interface_id, component)
            .await
    }
}

#[async_trait]
impl RpcHandler<vault::GetAssets> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        vault_id: VaultId,
    ) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
        info!("Plugin {} requested balance of {:?}", plugin_id, vault_id);
        self.vault_get_assets(vault_id).await
    }
}

#[async_trait]
impl RpcHandler<vault::Withdraw> for Host {
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

        self.vault_withdraw(vault, to, asset, amount).await
    }
}

#[async_trait]
impl RpcHandler<vault::GetDepositAddress> for Host {
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

        self.vault_get_deposit_address(vault_id, asset).await
    }
}

#[async_trait]
impl RpcHandler<vault::OnDeposit> for Host {
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

        self.vault_on_deposit(vault_id, asset).await
    }
}

#[async_trait]
impl RpcHandler<page::OnLoad> for Host {
    async fn invoke(&self, plugin_id: PluginId, interface_id: u32) -> Result<(), RpcErrorCode> {
        info!(
            "Plugin {} requested OnPageLoad for interface {}",
            plugin_id, interface_id
        );
        self.page_on_load(&plugin_id, interface_id).await
    }
}

#[async_trait]
impl RpcHandler<page::OnUpdate> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        (interface_id, event): (u32, page::PageEvent),
    ) -> Result<(), RpcErrorCode> {
        info!(
            "Plugin {} sent OnPageUpdate for interface {}: {:?}",
            plugin_id, interface_id, event
        );
        self.page_on_update(&plugin_id, interface_id, event).await
    }
}

#[async_trait]
impl RpcHandler<eth::BlockNumber> for Host {
    async fn invoke(&self, plugin_id: PluginId, _params: ()) -> Result<u64, RpcErrorCode> {
        info!("Plugin {} requested BlockNumber", plugin_id);
        self.eth_provider_block_number(&plugin_id).await
    }
}

#[async_trait]
impl RpcHandler<eth::Call> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: <eth::Call as RpcMethod>::Params,
    ) -> Result<<eth::Call as RpcMethod>::Output, RpcErrorCode> {
        info!(
            "Plugin {} requested Call with params {:?}",
            plugin_id, params
        );
        self.eth_provider_call(&plugin_id, params).await
    }
}

#[async_trait]
impl RpcHandler<eth::GetBalance> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: <eth::GetBalance as RpcMethod>::Params,
    ) -> Result<<eth::GetBalance as RpcMethod>::Output, RpcErrorCode> {
        info!(
            "Plugin {} requested GetBalance with params {:?}",
            plugin_id, params
        );
        self.eth_provider_get_balance(&plugin_id, params).await
    }
}
