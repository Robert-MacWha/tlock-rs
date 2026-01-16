use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex, Weak},
    time::Duration,
    usize,
};

use alloy::{primitives::U256, transports::http::reqwest};
use futures::channel::{mpsc::UnboundedSender, oneshot};
use thiserror::Error;
use tlock_hdk::{
    impl_host_rpc, impl_host_rpc_no_id,
    server::HostServer,
    tlock_api::{
        RpcMethod,
        caip::{self, AccountId, AssetId},
        component::Component,
        coordinator,
        domains::Domain,
        entities::{CoordinatorId, EntityId, EthProviderId, PageId, VaultId},
        eth, global, host, page, plugin, state,
        vault::{self},
    },
    wasmi_plugin_hdk::{self, instance_id::InstanceId, plugin::Plugin, plugin_id::PluginId},
    wasmi_plugin_pdk::rpc_message::{RpcError, RpcErrorContext},
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::host_state::{HostState, PluginData, PluginSource};

pub struct Host {
    plugins: Mutex<HashMap<PluginId, Plugin>>,
    plugin_sources: Mutex<HashMap<PluginId, PluginSource>>,
    entities: Mutex<HashMap<EntityId, PluginId>>,

    // TODO: Restrict these to a max size / otherwise prevent plugins from abusing storage
    state: Mutex<HashMap<(PluginId, String), Vec<u8>>>,
    locks: Mutex<HashMap<(PluginId, String), (InstanceId, Arc<event_listener::Event>)>>,

    interfaces: Mutex<HashMap<PageId, Component>>,

    // User requests awaiting user decisions
    user_requests: Mutex<Vec<UserRequest>>,
    user_request_senders: Mutex<HashMap<Uuid, oneshot::Sender<UserResponse>>>,

    events: Mutex<Vec<Event>>,
    observers: Mutex<Vec<UnboundedSender<()>>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UserRequest {
    EthProviderSelection {
        id: Uuid,
        plugin_id: PluginId,
        chain_id: caip::ChainId,
    },
    VaultSelection {
        id: Uuid,
        plugin_id: PluginId,
    },
    CoordinatorSelection {
        id: Uuid,
        plugin_id: PluginId,
    },
}

#[derive(Debug, Clone)]
pub struct Event {
    pub id: Uuid,
    pub message: String,
    pub level: host::NotifyLevel,
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub plugin: Option<String>,
}

const PLUGIN_TIMEOUT_SECS: u64 = 300;

impl UserRequest {
    pub fn id(&self) -> Uuid {
        match self {
            UserRequest::EthProviderSelection { id, .. } => id.clone(),
            UserRequest::VaultSelection { id, .. } => id.clone(),
            UserRequest::CoordinatorSelection { id, .. } => id.clone(),
        }
    }

    pub fn plugin_id(&self) -> PluginId {
        match self {
            UserRequest::EthProviderSelection { plugin_id, .. } => *plugin_id,
            UserRequest::VaultSelection { plugin_id, .. } => *plugin_id,
            UserRequest::CoordinatorSelection { plugin_id, .. } => *plugin_id,
        }
    }
}

#[derive(Debug, Clone)]
pub enum UserResponse {
    EthProvider(EthProviderId),
    Vault(VaultId),
    Coordinator(CoordinatorId),
}

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("reqwest error")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Pdk error")]
    PdkError(#[from] wasmi_plugin_hdk::plugin::PluginError),
    #[error("Rpc error")]
    RpcError(#[from] RpcError),
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
            plugin_sources: Mutex::new(HashMap::new()),
            entities: Mutex::new(HashMap::new()),
            state: Mutex::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
            interfaces: Mutex::new(HashMap::new()),
            user_requests: Mutex::new(Vec::new()),
            user_request_senders: Mutex::new(HashMap::new()),
            events: Mutex::new(Vec::new()),
            observers: Mutex::new(Vec::new()),
        }
    }

    pub async fn from_state(host_state: HostState) -> Result<Arc<Self>, PluginError> {
        let entities: HashMap<EntityId, PluginId> = host_state.entities.into_iter().collect();
        let state: HashMap<(PluginId, String), Vec<u8>> = host_state.state.into_iter().collect();

        let host = Self {
            plugins: Mutex::new(HashMap::new()),
            plugin_sources: Mutex::new(HashMap::new()),
            entities: Mutex::new(entities),
            state: Mutex::new(state),
            locks: Mutex::new(HashMap::new()),
            interfaces: Mutex::new(HashMap::new()),
            user_requests: Mutex::new(Vec::new()),
            user_request_senders: Mutex::new(HashMap::new()),
            events: Mutex::new(Vec::new()),
            observers: Mutex::new(Vec::new()),
        };
        let host = Arc::new(host);

        for plugin_data in host_state.plugins {
            host.load_plugin(plugin_data.source, &plugin_data.name)
                .await?;
        }

        Ok(host)
    }

    pub fn state(&self) -> HostState {
        let plugins = self.plugins.lock().unwrap();
        let plugin_sources = self.plugin_sources.lock().unwrap();

        let plugins_data = plugins
            .iter()
            .map(|(id, plugin)| PluginData {
                id: *id,
                name: plugin.name().to_string(),
                source: plugin_sources
                    .get(id)
                    .cloned()
                    .expect("Plugin source not found"),
            })
            .collect();

        HostState {
            plugins: plugins_data,
            entities: self.entities.lock().unwrap().clone().into_iter().collect(),
            state: self.state.lock().unwrap().clone().into_iter().collect(),
        }
    }

    pub fn subscribe(&self, tx: UnboundedSender<()>) {
        self.observers.lock().unwrap().push(tx);
    }

    fn notify_observers(&self) {
        let mut observers = self.observers.lock().unwrap();
        observers.retain(|tx| tx.unbounded_send(()).is_ok());
    }

    /// Creates a plugin from its source, register it, and calls its Init method
    pub async fn new_plugin(
        self: &Arc<Host>,
        source: PluginSource,
        name: &str,
    ) -> Result<PluginId, PluginError> {
        let plugin = self.load_plugin(source, name).await?;
        info!("Initializing plugin {}", plugin.id());

        let plugin_id = plugin.id();
        match plugin::Init.call_async(plugin.clone(), ()).await {
            Err(RpcError::MethodNotFound) => {
                info!("Plugin {} does not implement Init, skipping", plugin.id());
                self.log_event("Initialized", Some(name));
                Ok(plugin_id)
            }
            Err(e) => Err(e.into()),
            Ok(_) => {
                info!("Plugin {} initialized", plugin.id());
                self.log_event("Initialized", Some(name));
                Ok(plugin_id)
            }
        }
    }

    /// Loads a new plugin from its source and registers it
    async fn load_plugin(
        self: &Arc<Host>,
        source: PluginSource,
        name: &str,
    ) -> Result<Plugin, PluginError> {
        let server = self.get_server();
        let server = Arc::new(server);

        let wasm_bytes = source.as_bytes().await?;

        info!("Loading plugin '{}'...", name);
        let mut s = DefaultHasher::new();
        wasm_bytes.hash(&mut s);
        let id: u128 = s.finish().into();
        let id = PluginId::from(id);

        let plugin = Plugin::builder(name, wasm_bytes, server)
            .with_id(id)
            .with_timeout(Duration::from_secs(PLUGIN_TIMEOUT_SECS))
            .build()
            .await?;

        self.plugins
            .lock()
            .unwrap()
            .insert(plugin.id(), plugin.clone());

        self.plugin_sources.lock().unwrap().insert(id, source);
        info!("Loaded plugin '{}'", name);
        Ok(plugin)
    }

    pub fn get_server(self: &Arc<Host>) -> HostServer<Weak<Host>> {
        let weak_host = Arc::downgrade(self);
        HostServer::new(weak_host)
            .with_method(global::Ping, ping)
            .with_method(host::RegisterEntity, register_entity)
            .with_method(host::RequestEthProvider, request_eth_provider)
            .with_method(host::RequestVault, request_vault)
            .with_method(host::RequestCoordinator, request_coordinator)
            .with_method(host::Fetch, fetch)
            .with_method(host::Notify, notify)
            .with_method(state::ReadKey, read_key)
            .with_method(state::LockKey, lock_key)
            .with_method(state::SetKey, set_key)
            .with_method(state::UnlockKey, unlock_key)
            .with_method(host::SetPage, set_interface)
            .with_method(vault::GetAssets, vault_get_assets)
            .with_method(vault::Withdraw, vault_withdraw)
            .with_method(vault::GetDepositAddress, vault_get_deposit_address)
            // .with_method(vault::OnDeposit, vault_on_deposit)
            .with_method(page::OnLoad, page_on_load)
            .with_method(page::OnUpdate, page_on_update)
            .with_method(eth::ChainId, eth_provider_chain_id)
            .with_method(eth::BlockNumber, eth_provider_block_number)
            .with_method(eth::Call, eth_provider_call)
            .with_method(eth::GetBalance, eth_provider_get_balance)
            .with_method(eth::GasPrice, eth_provider_gas_price)
            .with_method(eth::GetTransactionCount, eth_transaction_count)
            .with_method(eth::SendRawTransaction, eth_send_raw_transaction)
            .with_method(eth::EstimateGas, eth_estimate_gas)
            .with_method(eth::GetTransactionReceipt, eth_get_transaction_receipt)
            .with_method(eth::GetBlock, eth_get_block)
            .with_method(eth::GetCode, eth_get_code)
            .with_method(eth::GetStorageAt, eth_get_storage_at)
            .with_method(eth::FeeHistory, eth_fee_history)
            .with_method(coordinator::GetAssets, coordinator_get_assets)
            .with_method(coordinator::GetSession, coordinator_get_session)
            .with_method(coordinator::Propose, coordinator_propose)
    }

    pub fn get_entities(&self) -> Vec<EntityId> {
        let entities = self.entities.lock().unwrap();
        entities.keys().cloned().collect()
    }

    pub fn get_plugins(&self) -> Vec<PluginId> {
        let plugins = self.plugins.lock().unwrap();
        plugins.keys().cloned().collect()
    }

    pub fn get_plugin(&self, plugin_id: &PluginId) -> Option<Plugin> {
        self.plugins.lock().unwrap().get(plugin_id).cloned()
    }

    pub fn get_entity_plugin_id(&self, entity_id: impl Into<EntityId>) -> Option<PluginId> {
        let entity_id = entity_id.into();
        let entities = self.entities.lock().unwrap();
        entities.get(&entity_id).cloned()
    }

    pub fn get_entity_plugin(&self, entity_id: impl Into<EntityId>) -> Option<Plugin> {
        let entity_id = entity_id.into();
        let plugin_id = self.get_entity_plugin_id(entity_id)?;
        self.get_plugin(&plugin_id)
    }

    pub fn get_interfaces(&self) -> HashMap<PageId, Component> {
        let interfaces = self.interfaces.lock().unwrap();
        interfaces.clone()
    }

    pub fn get_interface(&self, page_id: PageId) -> Option<Component> {
        let interfaces = self.interfaces.lock().unwrap();
        interfaces.get(&page_id).cloned()
    }

    pub fn get_user_requests(&self) -> Vec<UserRequest> {
        let requests = self.user_requests.lock().unwrap();
        requests.clone()
    }

    pub fn get_events(&self) -> Vec<Event> {
        let events = self.events.lock().unwrap();
        events.clone()
    }

    pub fn resolve_eth_provider_request(&self, request_id: Uuid, provider_id: EthProviderId) {
        self.resolve_user_request(request_id, UserResponse::EthProvider(provider_id));
    }

    pub fn resolve_vault_request(&self, request_id: Uuid, vault_id: VaultId) {
        self.resolve_user_request(request_id, UserResponse::Vault(vault_id));
    }

    pub fn resolve_coordinator_request(&self, request_id: Uuid, coordinator_id: CoordinatorId) {
        self.resolve_user_request(request_id, UserResponse::Coordinator(coordinator_id.into()));
    }

    pub fn deny_user_request(&self, request_id: Uuid) {
        //? Drop the sender to cancel the request
        self.user_request_senders
            .lock()
            .unwrap()
            .remove(&request_id);
    }

    async fn create_user_request<T, F>(
        &self,
        request: UserRequest,
        extract_response: F,
    ) -> Result<T, RpcError>
    where
        F: FnOnce(UserResponse) -> Option<T>,
    {
        let request_id = request.id();

        // Insert the request
        self.user_requests.lock().unwrap().push(request);

        // Construct a receiver for the response and await it
        let (sender, receiver) = oneshot::channel();
        self.user_request_senders
            .lock()
            .unwrap()
            .insert(request_id.clone(), sender);

        self.notify_observers();
        let resp = receiver.await;

        // Remove the request from the list
        self.user_requests
            .lock()
            .unwrap()
            .retain(|req| req.id() != request_id);

        let Ok(resp) = resp else {
            return Err(RpcError::Custom("Request Dropped".into()));
        };

        let Some(resp) = extract_response(resp) else {
            return Err(RpcError::Custom("Unexpected Response Type".into()));
        };

        Ok(resp)
    }

    fn resolve_user_request(&self, request_id: Uuid, resp: UserResponse) {
        let sender = self
            .user_request_senders
            .lock()
            .unwrap()
            .remove(&request_id);
        let Some(sender) = sender else {
            warn!("No sender found for user request {}", request_id);
            return;
        };

        if sender.send(resp).is_err() {
            warn!("Failed to send response for user request {}", request_id);
        }
    }

    /// ? Helper to get the plugin or return an RpcError if not found
    fn get_entity_plugin_error(&self, entity_id: impl Into<EntityId>) -> Result<Plugin, RpcError> {
        let entity_id = entity_id.into();
        let plugin = self
            .get_entity_plugin(entity_id)
            .context(format!("Entity {:?} not found", entity_id))?;

        Ok(plugin)
    }

    pub async fn ping_plugin(&self, plugin_id: &PluginId) -> Result<String, RpcError> {
        let plugin = self
            .get_plugin(plugin_id)
            .context(format!("Plugin {} not found", plugin_id))?;

        let resp = global::Ping
            .call_async(plugin, ())
            .await
            .context(format!("Error calling Ping on plugin {}", plugin_id))?;
        Ok(resp)
    }

    pub fn log_event(&self, event: &str, plugin: Option<&str>) {
        let mut log = self.events.lock().unwrap();
        log.push(Event {
            id: Uuid::new_v4(),
            message: event.to_string(),
            level: host::NotifyLevel::Trace,
            timestamp: chrono::Local::now(),
            plugin: plugin.map(|p| p.to_string()),
        });
    }
}

// TODO: Create a macro for these. It seens extremely possible, if a little
// fiddly.
impl Host {
    pub async fn ping(&self, _instance_id: &InstanceId, _params: ()) -> Result<String, RpcError> {
        Ok("Pong from host".to_string())
    }

    pub async fn register_entity(
        &self,
        instance_id: &InstanceId,
        domain: Domain,
    ) -> Result<EntityId, RpcError> {
        let entity_id: EntityId = match domain {
            Domain::EthProvider => EthProviderId::new().into(),
            Domain::Page => PageId::new().into(),
            Domain::Vault => VaultId::new().into(),
            Domain::Coordinator => CoordinatorId::new().into(),
        };

        let mut entities = self.entities.lock().unwrap();
        entities.insert(entity_id, instance_id.plugin);
        Ok(entity_id)
    }

    pub async fn request_eth_provider(
        &self,
        instance_id: &InstanceId,
        chain_id: caip::ChainId,
    ) -> Result<EthProviderId, RpcError> {
        let request = UserRequest::EthProviderSelection {
            id: Uuid::new_v4(),
            plugin_id: instance_id.plugin,
            chain_id,
        };

        self.create_user_request(request, |resp| match resp {
            UserResponse::EthProvider(selected_provider) => Some(selected_provider),
            _ => None,
        })
        .await
    }

    pub async fn request_vault(
        &self,
        instance_id: &InstanceId,
        _params: (),
    ) -> Result<VaultId, RpcError> {
        let request = UserRequest::VaultSelection {
            id: Uuid::new_v4(),
            plugin_id: instance_id.plugin,
        };

        self.create_user_request(request, |resp| match resp {
            UserResponse::Vault(selected_vault) => Some(selected_vault),
            _ => None,
        })
        .await
    }

    pub async fn request_coordinator(
        &self,
        instance_id: &InstanceId,
        _params: (),
    ) -> Result<CoordinatorId, RpcError> {
        let request = UserRequest::CoordinatorSelection {
            id: Uuid::new_v4(),
            plugin_id: instance_id.plugin,
        };

        self.create_user_request(request, |resp| match resp {
            UserResponse::Coordinator(selected_coordinator) => Some(selected_coordinator),
            _ => None,
        })
        .await
    }

    pub async fn fetch(
        &self,
        _instance_id: &InstanceId,
        req: host::Request,
    ) -> Result<Result<Vec<u8>, String>, RpcError> {
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

        // TODO: Handle errors properly
        let resp = request
            .send()
            .await
            .context("Failed to send HTTP request")?;
        let bytes = resp
            .bytes()
            .await
            .context("Failed to read response bytes")?;
        Ok(Ok(bytes.to_vec()))
    }

    pub async fn notify(
        &self,
        instance_id: &InstanceId,
        params: (host::NotifyLevel, String),
    ) -> Result<(), RpcError> {
        {
            let (level, message) = params;

            let plugin_name = match self.get_plugin(&instance_id.plugin) {
                Some(plugin) => plugin.name().to_string(),
                None => "Unknown Plugin".to_string(),
            };

            self.events.lock().unwrap().push(Event {
                id: Uuid::new_v4(),
                message,
                level,
                timestamp: chrono::Local::now(),
                plugin: Some(plugin_name),
            });
        }

        self.notify_observers();
        Ok(())
    }

    pub async fn read_key(
        &self,
        instance_id: &InstanceId,
        key: String,
    ) -> Result<Vec<u8>, RpcError> {
        let state_key = (instance_id.plugin, key);

        //? Iteratively wait for the lock to be released and our turn to access the key
        loop {
            let listener = {
                let locks = self.locks.lock().unwrap();
                match locks.get(&state_key) {
                    Some((holder, _)) if holder == instance_id => {
                        return Err(RpcError::custom(format!(
                            "Deadlock: instance already holds lock on key '{}'",
                            state_key.1
                        )));
                    }
                    Some((_holder, event)) => {
                        // Different holder - wait
                        event.listen()
                    }
                    None => {
                        // Not held, read it
                        let state = self.state.lock().unwrap();
                        return Ok(state.get(&state_key).cloned().unwrap_or_default());
                    }
                }
            };
            listener.await;
        }
    }

    pub async fn lock_key(
        &self,
        instance_id: &InstanceId,
        key: String,
    ) -> Result<Vec<u8>, RpcError> {
        let state_key = (instance_id.plugin, key);

        //? Iteratively wait for the lock to be released and our turn to access the key
        loop {
            let listener = {
                let mut locks = self.locks.lock().unwrap();

                match locks.get(&state_key) {
                    Some((holder, _)) if holder == instance_id => {
                        return Err(RpcError::custom(format!(
                            "Deadlock: instance already holds lock on key '{}'",
                            state_key.1
                        )));
                    }
                    Some((_holder, event)) => {
                        // Different holder - wait
                        event.listen()
                    }
                    None => {
                        // Not held, acquire it
                        locks.insert(
                            state_key.clone(),
                            (*instance_id, Arc::new(event_listener::Event::new())),
                        );
                        let state = self.state.lock().unwrap();
                        return Ok(state.get(&state_key).cloned().unwrap_or_default());
                    }
                }
            };
            listener.await;
        }
    }

    pub async fn set_key(
        &self,
        instance_id: &InstanceId,
        params: (String, Vec<u8>),
    ) -> Result<Result<(), state::SetError>, RpcError> {
        let (key, value) = params;
        let state_key = (instance_id.plugin, key);

        {
            let locks = self.locks.lock().unwrap();
            match locks.get(&state_key) {
                Some((holder, _)) if holder == instance_id => {}
                _ => return Ok(Err(state::SetError::KeyNotLocked)),
            }
        }

        let mut state = self.state.lock().unwrap();
        state.insert(state_key, value);
        Ok(Ok(()))
    }

    pub async fn unlock_key(
        &self,
        instance_id: &InstanceId,
        key: String,
    ) -> Result<Result<(), state::UnlockError>, RpcError> {
        let state_key = (instance_id.plugin, key);

        let event = {
            let mut locks = self.locks.lock().unwrap();
            match locks.get(&state_key) {
                Some((holder, _)) if holder == instance_id => locks.remove(&state_key).unwrap().1,
                _ => return Ok(Err(state::UnlockError::KeyNotLocked)),
            }
        };

        event.notify(usize::MAX);
        Ok(Ok(()))
    }

    /// Unlocks all locks held by an instance
    pub async fn unlock_instance(&self, instance_id: &InstanceId) {
        let mut locks = self.locks.lock().unwrap();
        let keys_to_remove: Vec<_> = locks
            .iter()
            .filter(|(_, (holder, _))| holder == instance_id)
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            if let Some((_, event)) = locks.remove(&key) {
                info!(
                    "Plugin {} unlocking key '{}' due to instance drop",
                    instance_id.plugin, key.1
                );
                event.notify(usize::MAX);
            }
        }
    }

    pub async fn set_interface(
        &self,
        _instance_id: &InstanceId,
        params: (PageId, Component),
    ) -> Result<(), RpcError> {
        let (page_id, component) = params;
        self.interfaces.lock().unwrap().insert(page_id, component);
        self.notify_observers();
        Ok(())
    }

    pub async fn vault_get_assets(
        &self,
        vault_id: VaultId,
    ) -> Result<Vec<(AssetId, U256)>, RpcError> {
        let plugin = self.get_entity_plugin_error(vault_id)?;

        let balance = vault::GetAssets
            .call_async(plugin, vault_id)
            .await
            .context("Error calling BalanceOf")?;
        Ok(balance)
    }

    pub async fn vault_withdraw(
        &self,
        params: (VaultId, AccountId, AssetId, U256),
    ) -> Result<(), RpcError> {
        let (vault_id, to, asset, amount) = params;
        let plugin = self.get_entity_plugin_error(vault_id)?;

        vault::Withdraw
            .call_async(plugin, (vault_id, to, asset, amount))
            .await
            .context("Error calling Withdraw")?;
        Ok(())
    }

    pub async fn vault_get_deposit_address(
        &self,
        params: (VaultId, AssetId),
    ) -> Result<AccountId, RpcError> {
        let (vault_id, asset) = params;
        let plugin = self.get_entity_plugin_error(vault_id)?;

        let result = vault::GetDepositAddress
            .call_async(plugin, (vault_id, asset))
            .await
            .context("Error calling GetDepositAddress")?;
        Ok(result)
    }

    pub async fn page_on_load(&self, page_id: PageId) -> Result<(), RpcError> {
        let plugin = self.get_entity_plugin_error(page_id)?;

        page::OnLoad
            .call_async(plugin, page_id)
            .await
            .context("Error calling OnPageLoad")?;
        Ok(())
    }

    pub async fn page_on_update(&self, params: (PageId, page::PageEvent)) -> Result<(), RpcError> {
        let (page_id, event) = params;
        let plugin = self.get_entity_plugin_error(page_id)?;

        page::OnUpdate
            .call_async(plugin, (page_id, event))
            .await
            .context("Error calling OnPageUpdate")?;
        Ok(())
    }

    pub async fn eth_provider_chain_id(
        &self,
        provider_id: EthProviderId,
    ) -> Result<U256, RpcError> {
        let plugin = self.get_entity_plugin_error(provider_id)?;

        let chain_id = eth::ChainId
            .call_async(plugin, provider_id)
            .await
            .context("Error calling ChainId")?;
        Ok(chain_id)
    }

    pub async fn eth_provider_block_number(
        &self,
        provider_id: EthProviderId,
    ) -> Result<u64, RpcError> {
        let plugin = self.get_entity_plugin_error(provider_id)?;

        let block_number = eth::BlockNumber
            .call_async(plugin, provider_id)
            .await
            .context("Error calling BlockNumber")?;
        Ok(block_number)
    }

    pub async fn eth_provider_call(
        &self,
        params: <eth::Call as RpcMethod>::Params,
    ) -> Result<<eth::Call as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let resp = eth::Call
            .call_async(plugin, params)
            .await
            .context("Error calling Call")?;
        Ok(resp)
    }

    pub async fn eth_provider_get_balance(
        &self,
        params: <eth::GetBalance as RpcMethod>::Params,
    ) -> Result<<eth::GetBalance as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let resp = eth::GetBalance
            .call_async(plugin, params)
            .await
            .context("Error calling GetBalance")?;
        Ok(resp)
    }

    pub async fn eth_provider_gas_price(
        &self,
        provider_id: EthProviderId,
    ) -> Result<u128, RpcError> {
        let plugin = self.get_entity_plugin_error(provider_id)?;

        let gas_price = eth::GasPrice
            .call_async(plugin, provider_id)
            .await
            .context("Error calling GasPrice")?;
        Ok(gas_price)
    }

    pub async fn eth_transaction_count(
        &self,
        params: <eth::GetTransactionCount as RpcMethod>::Params,
    ) -> Result<<eth::GetTransactionCount as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let resp = eth::GetTransactionCount
            .call_async(plugin, params)
            .await
            .context("Error calling GetTransactionCount")?;
        Ok(resp)
    }

    pub async fn eth_send_raw_transaction(
        &self,
        params: <eth::SendRawTransaction as RpcMethod>::Params,
    ) -> Result<<eth::SendRawTransaction as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let tx_hash = eth::SendRawTransaction
            .call_async(plugin, params)
            .await
            .context("Error calling SendRawTransaction")?;
        Ok(tx_hash)
    }

    pub async fn eth_estimate_gas(
        &self,
        params: <eth::EstimateGas as RpcMethod>::Params,
    ) -> Result<<eth::EstimateGas as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let gas_estimate = eth::EstimateGas
            .call_async(plugin, params)
            .await
            .context("Error calling EstimateGas")?;
        Ok(gas_estimate)
    }

    pub async fn eth_get_transaction_receipt(
        &self,
        params: <eth::GetTransactionReceipt as RpcMethod>::Params,
    ) -> Result<<eth::GetTransactionReceipt as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let receipt = eth::GetTransactionReceipt
            .call_async(plugin, params)
            .await
            .context("Error calling GetTransactionReceipt")?;
        Ok(receipt)
    }

    pub async fn eth_get_block(
        &self,
        params: <eth::GetBlock as RpcMethod>::Params,
    ) -> Result<<eth::GetBlock as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let block = eth::GetBlock
            .call_async(plugin, params)
            .await
            .context("Error calling GetBlock")?;
        Ok(block)
    }

    pub async fn eth_get_code(
        &self,
        params: <eth::GetCode as RpcMethod>::Params,
    ) -> Result<<eth::GetCode as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let code = eth::GetCode
            .call_async(plugin, params)
            .await
            .context("Error calling GetCode")?;
        Ok(code)
    }

    pub async fn eth_get_storage_at(
        &self,
        params: <eth::GetStorageAt as RpcMethod>::Params,
    ) -> Result<<eth::GetStorageAt as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let storage = eth::GetStorageAt
            .call_async(plugin, params)
            .await
            .context("Error calling GetStorageAt")?;
        Ok(storage)
    }

    pub async fn eth_fee_history(
        &self,
        params: <eth::FeeHistory as RpcMethod>::Params,
    ) -> Result<<eth::FeeHistory as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let history = eth::FeeHistory
            .call_async(plugin, params)
            .await
            .context("Error calling FeeHistory")?;
        Ok(history)
    }

    pub async fn coordinator_get_assets(
        &self,
        params: <coordinator::GetAssets as RpcMethod>::Params,
    ) -> Result<<coordinator::GetAssets as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let assets = coordinator::GetAssets
            .call_async(plugin, params)
            .await
            .context("Error calling GetAssets")?;
        Ok(assets)
    }

    pub async fn coordinator_get_session(
        &self,
        params: <coordinator::GetSession as RpcMethod>::Params,
    ) -> Result<<coordinator::GetSession as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let session = coordinator::GetSession
            .call_async(plugin, params)
            .await
            .context("Error calling GetSession")?;
        Ok(session)
    }

    pub async fn coordinator_propose(
        &self,
        params: <coordinator::Propose as RpcMethod>::Params,
    ) -> Result<<coordinator::Propose as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let result = coordinator::Propose
            .call_async(plugin, params)
            .await
            .context("Error calling Propose")?;
        Ok(result)
    }
}

// Macro invocations to implement the host RPC methods
//
// Because some host methods rely on the entity ID, while others are ID-less, we
// have two seperate macros. Functionally they do the same thing, it just cleans
// up the ID-less function signatures for external callers so they don't need to
// pass a dummy ID.
impl_host_rpc!(Host, global::Ping, ping);
impl_host_rpc!(Host, host::RegisterEntity, register_entity);
impl_host_rpc!(Host, host::RequestEthProvider, request_eth_provider);
impl_host_rpc!(Host, host::RequestVault, request_vault);
impl_host_rpc!(Host, host::RequestCoordinator, request_coordinator);
impl_host_rpc!(Host, host::Fetch, fetch);
impl_host_rpc!(Host, state::ReadKey, read_key);
impl_host_rpc!(Host, state::LockKey, lock_key);
impl_host_rpc!(Host, state::SetKey, set_key);
impl_host_rpc!(Host, state::UnlockKey, unlock_key);
impl_host_rpc!(Host, host::SetPage, set_interface);
impl_host_rpc!(Host, host::Notify, notify);
impl_host_rpc_no_id!(Host, vault::GetAssets, vault_get_assets);
impl_host_rpc_no_id!(Host, vault::Withdraw, vault_withdraw);
impl_host_rpc_no_id!(Host, vault::GetDepositAddress, vault_get_deposit_address);
// impl_host_rpc_no_id!(Host, vault::OnDeposit, vault_on_deposit);
impl_host_rpc_no_id!(Host, page::OnLoad, page_on_load);
impl_host_rpc_no_id!(Host, page::OnUpdate, page_on_update);
impl_host_rpc_no_id!(Host, eth::ChainId, eth_provider_chain_id);
impl_host_rpc_no_id!(Host, eth::BlockNumber, eth_provider_block_number);
impl_host_rpc_no_id!(Host, eth::Call, eth_provider_call);
impl_host_rpc_no_id!(Host, eth::GetBalance, eth_provider_get_balance);
impl_host_rpc_no_id!(Host, eth::GasPrice, eth_provider_gas_price);
impl_host_rpc_no_id!(Host, eth::GetTransactionCount, eth_transaction_count);
impl_host_rpc_no_id!(Host, eth::SendRawTransaction, eth_send_raw_transaction);
impl_host_rpc_no_id!(Host, eth::EstimateGas, eth_estimate_gas);
impl_host_rpc_no_id!(
    Host,
    eth::GetTransactionReceipt,
    eth_get_transaction_receipt
);
impl_host_rpc_no_id!(Host, eth::GetBlock, eth_get_block);
impl_host_rpc_no_id!(Host, eth::GetCode, eth_get_code);
impl_host_rpc_no_id!(Host, eth::GetStorageAt, eth_get_storage_at);
impl_host_rpc_no_id!(Host, eth::FeeHistory, eth_fee_history);
impl_host_rpc_no_id!(Host, coordinator::GetAssets, coordinator_get_assets);
impl_host_rpc_no_id!(Host, coordinator::GetSession, coordinator_get_session);
impl_host_rpc_no_id!(Host, coordinator::Propose, coordinator_propose);
