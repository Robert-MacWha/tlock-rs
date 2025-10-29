use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex, Weak},
};

use alloy::{primitives::U256, transports::http::reqwest};
use tlock_hdk::{
    impl_host_rpc, impl_host_rpc_no_id,
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId},
        component::Component,
        domains::Domain,
        entities::{EntityId, EthProviderId, PageId, VaultId},
        eth, global, host, page, plugin,
        vault::{self},
    },
    tlock_pdk::server::ServerBuilder,
    wasmi_hdk::plugin::{Plugin, PluginError, PluginId},
    wasmi_pdk::{rpc_message::RpcError, server::Server},
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

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
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
        let server = self.get_server();
        let server = Arc::new(server);

        info!("Loading plugin '{}'...", name);
        let mut s = DefaultHasher::new();
        wasm_bytes.hash(&mut s);
        let id = s.finish().to_string().into();
        let plugin = Plugin::new(name, &id, wasm_bytes.to_vec(), server)
            .map_err(|e| PluginError::SpawnError(e.into()))?;

        info!("Registering plugin '{}' with id {}", name, id);
        self.register_plugin(plugin).await?;
        info!("Loaded plugin '{}'", name);
        Ok(id)
    }

    pub fn get_server(self: &Arc<Host>) -> Server<(Option<PluginId>, Weak<Host>)> {
        ServerBuilder::new(Arc::new((None, Arc::downgrade(self))))
            .with_method(global::Ping, ping)
            .with_method(host::RegisterEntity, register_entity)
            .with_method(host::Fetch, fetch)
            .with_method(host::GetState, get_state)
            .with_method(host::SetState, set_state)
            .with_method(host::SetInterface, set_interface)
            .with_method(vault::GetAssets, vault_get_assets)
            .with_method(vault::Withdraw, vault_withdraw)
            .with_method(vault::GetDepositAddress, vault_get_deposit_address)
            .with_method(vault::OnDeposit, vault_on_deposit)
            .with_method(page::OnLoad, page_on_load)
            .with_method(page::OnUpdate, page_on_update)
            .with_method(eth::BlockNumber, eth_provider_block_number)
            .with_method(eth::Call, eth_provider_call)
            .with_method(eth::GetBalance, eth_provider_get_balance)
            .finish()
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
            Err(PluginError::RpcError(RpcError::MethodNotFound)) => {
                info!(
                    "Plugin {} does not implement Init, skipping",
                    new_plugin.id()
                );
                Ok(())
            }
            Err(e) => {
                warn!("Error calling Init on plugin {}: {:?}", new_plugin.id(), e);
                Err(e)
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

    ///? Helper to get the plugin or return an RpcError if not found
    fn get_entity_plugin_error(&self, entity_id: &EntityId) -> Result<Arc<Plugin>, RpcError> {
        let plugin = self.get_entity_plugin(entity_id).ok_or_else(|| {
            warn!("Entity {:?} not found", entity_id);
            RpcError::InvalidParams
        })?;
        Ok(plugin)
    }

    pub async fn ping_plugin(&self, plugin_id: &PluginId) -> Result<String, RpcError> {
        let plugin = if let Some(plugin) = self.get_plugin(plugin_id) {
            plugin
        } else {
            warn!("Plugin {} not found", plugin_id);
            return Err(RpcError::InvalidParams);
        };

        let resp = global::Ping.call(plugin, ()).await.map_err(|e| {
            warn!("Error calling Ping on plugin {}: {:?}", plugin_id, e);
            e.as_rpc_code()
        })?;
        Ok(resp)
    }
}

impl Host {
    pub async fn ping(&self, _plugin_id: &PluginId, _params: ()) -> Result<String, RpcError> {
        Ok("Pong from host".to_string())
    }

    pub async fn register_entity(
        &self,
        plugin_id: &PluginId,
        entity_id: EntityId,
    ) -> Result<(), RpcError> {
        let mut entities = self.entities.lock().unwrap();
        if let Some(existing_plugin_id) = entities.get(&entity_id) {
            if existing_plugin_id == plugin_id {
                return Ok(());
            } else {
                warn!(
                    "Entity {:?} is already registered by plugin {}",
                    entity_id, existing_plugin_id
                );
                return Err(RpcError::InvalidParams);
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

    pub async fn fetch(
        &self,
        plugin_id: &PluginId,
        req: host::Request,
    ) -> Result<Result<Vec<u8>, String>, RpcError> {
        info!("Plugin {} requested fetch: {:?}", plugin_id, req);

        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in req.headers.iter() {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_bytes(value),
            ) {
                headers.insert(name, val);
            }
        }

        let client = reqwest::Client::new();
        let body = req.body.clone().unwrap_or_default();
        let request = match req.method.to_lowercase().as_str() {
            "get" => client.get(req.url.clone()).headers(headers),
            "post" => client
                .post(req.url.clone())
                .headers(headers)
                .body(body.clone()),
            _ => {
                warn!("Unsupported HTTP method: {}", req.method);
                return Ok(Err("Unsupported HTTP method".to_string()));
            }
        };

        info!("Sending request: {:?}", request);
        let resp = request.send().await.unwrap();
        // let resp = request.send().await.map_err(|e| Ok(Err(e.to_string())))?;
        info!("Received response: {:?}", resp);
        let bytes = resp.bytes().await.unwrap();
        // let bytes = resp.bytes().await.map_err(|e| Ok(Err(e.to_string())))?;
        info!("Response bytes: {:?}", bytes);
        Ok(Ok(bytes.to_vec()))
    }

    pub async fn get_state(
        &self,
        plugin_id: &PluginId,
        _params: (),
    ) -> Result<Option<Vec<u8>>, RpcError> {
        Ok(self.state.lock().unwrap().get(plugin_id).cloned())
    }

    pub async fn set_state(
        &self,
        plugin_id: &PluginId,
        state_data: Vec<u8>,
    ) -> Result<(), RpcError> {
        self.state
            .lock()
            .unwrap()
            .insert(plugin_id.clone(), state_data);
        Ok(())
    }

    pub async fn set_interface(
        &self,
        _plugin_id: &PluginId,
        params: (u32, Component),
    ) -> Result<(), RpcError> {
        let (interface_id, component) = params;
        self.interfaces
            .lock()
            .unwrap()
            .insert(interface_id, component);
        Ok(())
    }

    pub async fn vault_get_assets(
        &self,
        vault_id: VaultId,
    ) -> Result<Vec<(AssetId, U256)>, RpcError> {
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
        params: (VaultId, AccountId, AssetId, U256),
    ) -> Result<Result<(), String>, RpcError> {
        let (vault_id, to, asset, amount) = params;
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
        params: (VaultId, AssetId),
    ) -> Result<Result<AccountId, String>, RpcError> {
        let (vault_id, asset) = params;
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

    pub async fn vault_on_deposit(&self, params: (VaultId, AssetId)) -> Result<(), RpcError> {
        let (vault_id, asset) = params;
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

    pub async fn page_on_load(&self, params: (PageId, u32)) -> Result<(), RpcError> {
        let (page_id, interface_id) = params;
        let plugin = self.get_entity_plugin_error(&page_id.as_entity_id())?;

        page::OnLoad
            .call(plugin, (page_id, interface_id))
            .await
            .map_err(|e| {
                warn!("Error calling OnPageLoad: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(())
    }

    pub async fn page_on_update(
        &self,
        params: (PageId, u32, page::PageEvent),
    ) -> Result<(), RpcError> {
        let (page_id, interface_id, event) = params;
        let plugin = self.get_entity_plugin_error(&page_id.as_entity_id())?;

        page::OnUpdate
            .call(plugin, (page_id, interface_id, event))
            .await
            .map_err(|e| {
                warn!("Error calling OnPageUpdate: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(())
    }

    pub async fn eth_provider_block_number(
        &self,
        provider_id: EthProviderId,
    ) -> Result<u64, RpcError> {
        let plugin = self.get_entity_plugin_error(&provider_id.as_entity_id())?;

        let block_number = eth::BlockNumber
            .call(plugin, provider_id)
            .await
            .map_err(|e| {
                warn!("Error calling BlockNumber: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(block_number)
    }

    pub async fn eth_provider_call(
        &self,
        params: <eth::Call as RpcMethod>::Params,
    ) -> Result<<eth::Call as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(&params.0.as_entity_id())?;

        let resp = eth::Call.call(plugin, params).await.map_err(|e| {
            warn!("Error calling Call: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(resp)
    }

    pub async fn eth_provider_get_balance(
        &self,
        params: <eth::GetBalance as RpcMethod>::Params,
    ) -> Result<<eth::GetBalance as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(&params.0.as_entity_id())?;

        let resp = eth::GetBalance.call(plugin, params).await.map_err(|e| {
            warn!("Error calling GetBalance: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(resp)
    }
}

impl_host_rpc!(Host, global::Ping, ping);
impl_host_rpc!(Host, host::RegisterEntity, register_entity);
impl_host_rpc!(Host, host::Fetch, fetch);
impl_host_rpc!(Host, host::GetState, get_state);
impl_host_rpc!(Host, host::SetState, set_state);
impl_host_rpc!(Host, host::SetInterface, set_interface);
impl_host_rpc_no_id!(Host, vault::GetAssets, vault_get_assets);
impl_host_rpc_no_id!(Host, vault::Withdraw, vault_withdraw);
impl_host_rpc_no_id!(Host, vault::GetDepositAddress, vault_get_deposit_address);
impl_host_rpc_no_id!(Host, vault::OnDeposit, vault_on_deposit);
impl_host_rpc_no_id!(Host, page::OnLoad, page_on_load);
impl_host_rpc_no_id!(Host, page::OnUpdate, page_on_update);
impl_host_rpc_no_id!(Host, eth::BlockNumber, eth_provider_block_number);
impl_host_rpc_no_id!(Host, eth::Call, eth_provider_call);
impl_host_rpc_no_id!(Host, eth::GetBalance, eth_provider_get_balance);

// // TODO: I can totally use a macro for all this boilerplate
// impl_rpc_handler!(Host, global::Ping, |self, plugin_id, _params| {
//     info!("[host_func] Plugin {} sent ping", plugin_id);
//     self.ping()
// });

// impl_rpc_handler!(Host, host::RegisterEntity, |self, plugin_id, entity_id| {
//     info!(
//         "[host_func] Plugin {} requested registration of entity {:?}",
//         plugin_id, entity_id
//     );
//     self.register_entity(&plugin_id, entity_id)
// });

// impl_rpc_handler!(Host, host::Fetch, |self, plugin_id, req| {
//     info!(
//         "[host_func] Plugin {} requested fetch: {:?}",
//         plugin_id, req
//     );
//     Ok(self.fetch(&plugin_id, req).await)
// });

// impl_rpc_handler!(Host, host::GetState, |self, plugin_id, _params| {
//     info!("[host_func] Plugin {} requested its state", plugin_id);
//     self.get_state(&plugin_id).await
// });

// impl_rpc_handler!(Host, host::SetState, |self, plugin_id, state_data| {
//     info!(
//         "[host_func] Plugin {} requested to set its state",
//         plugin_id
//     );
//     self.set_state(&plugin_id, state_data).await
// });

// impl_rpc_handler!(Host, host::SetInterface, |self, plugin_id, params| {
//     info!(
//         "[host_func] Plugin {} requested to set interface {:?}",
//         plugin_id, params
//     );
//     let (interface_id, component) = params;
//     self.set_interface(&plugin_id, interface_id, component)
//         .await
// });

// impl_rpc_handler!(Host, vault::GetAssets, |self, plugin_id, vault_id| {
//     info!(
//         "[host_func] Plugin {} requested balance of {:?}",
//         plugin_id, vault_id
//     );
//     self.vault_get_assets(vault_id).await
// });

// impl_rpc_handler!(Host, vault::Withdraw, |self, plugin_id, params| {
//     let (vault, to, asset, amount) = params;
//     info!(
//         "[host_func] Plugin {} requested transfer of {} {:?} from vault {:?} to {:?}",
//         plugin_id, amount, asset, vault, to
//     );
//     self.vault_withdraw(vault, to, asset, amount).await
// });

// impl_rpc_handler!(Host, vault::GetDepositAddress, |self, plugin_id, params| {
//     let (vault, asset) = params;
//     info!(
//         "[host_func] Plugin {} requested deposit address for asset {:?} in vault {:?}",
//         plugin_id, asset, vault
//     );
//     self.vault_get_deposit_address(vault, asset).await
// });

// impl_rpc_handler!(Host, vault::OnDeposit, |self, plugin_id, params| {
//     let (vault_id, asset) = params;
//     info!(
//         "[host_func] Plugin {} notified of receipt of asset {:?} in vault {:?}",
//         plugin_id, asset, vault_id
//     );
//     self.vault_on_deposit(vault_id, asset).await
// });

// impl_rpc_handler!(Host, page::OnLoad, |self, plugin_id, interface_id| {
//     info!(
//         "[host_func] Plugin {} requested OnPageLoad for interface {}",
//         plugin_id, interface_id
//     );
//     self.page_on_load(&plugin_id, interface_id).await
// });

// impl_rpc_handler!(Host, page::OnUpdate, |self, plugin_id, params| {
//     let (interface_id, event) = params;
//     info!(
//         "[host_func] Plugin {} requested OnPageUpdate for interface {}: {:?}",
//         plugin_id, interface_id, event
//     );
//     self.page_on_update(&plugin_id, interface_id, event).await
// });

// impl_rpc_handler!(Host, eth::BlockNumber, |self, plugin_id, _params| {
//     info!("[host_func] Plugin {} requested BlockNumber", plugin_id);
//     self.eth_provider_block_number(&plugin_id).await
// });

// impl_rpc_handler!(Host, eth::Call, |self, plugin_id, params| {
//     info!(
//         "[host_func] Plugin {} requested Call with params {:?}",
//         plugin_id, params
//     );
//     self.eth_provider_call(&plugin_id, params).await
// });

// impl_rpc_handler!(Host, eth::GetBalance, |self, plugin_id, params| {
//     info!(
//         "[host_func] Plugin {} requested GetBalance with params {:?}",
//         plugin_id, params
//     );
//     self.eth_provider_get_balance(&plugin_id, params).await
// });
