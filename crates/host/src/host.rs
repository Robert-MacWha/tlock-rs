use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex, Weak},
};

use alloy::{primitives::U256, transports::http::reqwest};
use futures::channel::{mpsc::UnboundedSender, oneshot};
use tlock_hdk::{
    impl_host_rpc, impl_host_rpc_no_id,
    server::HostServer,
    tlock_api::{
        RpcMethod,
        caip::{self, AccountId, AssetId},
        component::Component,
        domains::Domain,
        entities::{CoordinatorId, EntityId, EthProviderId, PageId, VaultId},
        eth, global, host, page, plugin,
        vault::{self},
    },
    wasmi_plugin_hdk::plugin::{Plugin, PluginError, PluginId},
    wasmi_plugin_pdk::rpc_message::RpcError,
};
use tracing::{info, warn};
use uuid::Uuid;

pub struct Host {
    plugins: Mutex<HashMap<PluginId, Arc<Plugin>>>,
    entities: Mutex<HashMap<EntityId, PluginId>>,

    // TODO: Restrict these to a max size / otherwise prevent plugins from abusing storage
    state: Mutex<HashMap<PluginId, Vec<u8>>>,
    interfaces: Mutex<HashMap<PageId, Component>>,

    // User requests awaiting user decisions
    user_requests: Mutex<Vec<UserRequest>>,
    user_request_senders: Mutex<HashMap<Uuid, oneshot::Sender<UserResponse>>>,

    observers: Mutex<Vec<UnboundedSender<()>>>,
    event_log: Mutex<Vec<String>>,
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
}

#[derive(Debug, Clone)]
pub enum UserResponse {
    EthProvider(EthProviderId),
    Vault(VaultId),
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
            state: Mutex::new(HashMap::new()),
            interfaces: Mutex::new(HashMap::new()),
            user_requests: Mutex::new(Vec::new()),
            user_request_senders: Mutex::new(HashMap::new()),
            observers: Mutex::new(Vec::new()),
            event_log: Mutex::new(Vec::new()),
        }
    }

    pub fn subscribe(&self, observer: UnboundedSender<()>) {
        let mut observers = self.observers.lock().unwrap();
        observers.push(observer);
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
        let id: u128 = s.finish().into();
        let id = PluginId::from(id);
        let plugin = Plugin::new(name, wasm_bytes.to_vec(), server)
            .map_err(|e| PluginError::SpawnError(e.into()))?
            .with_id(id);

        info!("Registering plugin '{}' with id {}", name, id);
        self.register_plugin(plugin).await?;
        info!("Loaded plugin '{}'", name);
        Ok(id)
    }

    pub fn get_server(self: &Arc<Host>) -> HostServer<Weak<Host>> {
        let weak_host = Arc::downgrade(self);
        HostServer::new(weak_host)
            .with_method(global::Ping, ping)
            .with_method(host::RegisterEntity, register_entity)
            .with_method(host::RequestEthProvider, request_eth_provider)
            .with_method(host::RequestVault, request_vault)
            .with_method(host::Fetch, fetch)
            .with_method(host::GetState, get_state)
            .with_method(host::SetState, set_state)
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
                self.notify_observers();
                Ok(())
            }
            Err(e) => {
                warn!("Error calling Init on plugin {}: {:?}", new_plugin.id(), e);
                self.notify_observers();
                Err(e)
            }
            Ok(_) => {
                info!("Plugin {} initialized", new_plugin.id());
                self.notify_observers();
                Ok(())
            }
        }
    }

    pub fn get_entities(&self) -> Vec<EntityId> {
        let entities = self.entities.lock().unwrap();
        entities.keys().cloned().collect()
    }

    pub fn get_plugins(&self) -> Vec<PluginId> {
        let plugins = self.plugins.lock().unwrap();
        plugins.keys().cloned().collect()
    }

    pub fn get_plugin(&self, plugin_id: &PluginId) -> Option<Arc<Plugin>> {
        let plugins = self.plugins.lock().unwrap();
        plugins.get(plugin_id).cloned()
    }

    pub fn get_entity_plugin_id(&self, entity_id: impl Into<EntityId>) -> Option<PluginId> {
        let entities = self.entities.lock().unwrap();
        entities.get(&entity_id.into()).cloned()
    }

    pub fn get_entity_plugin(&self, entity_id: impl Into<EntityId>) -> Option<Arc<Plugin>> {
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

    pub fn get_event_log(&self) -> Vec<String> {
        let log = self.event_log.lock().unwrap();
        log.clone()
    }

    pub fn resolve_user_request(&self, request_id: Uuid, provider_id: EthProviderId) {
        let sender = self
            .user_request_senders
            .lock()
            .unwrap()
            .remove(&request_id);
        let Some(sender) = sender else {
            warn!("No sender found for user request {}", request_id);
            return;
        };

        if sender.send(UserResponse::EthProvider(provider_id)).is_err() {
            warn!("Failed to send response for user request {}", request_id);
        }
    }

    pub fn resolve_vault_request(&self, request_id: Uuid, vault_id: VaultId) {
        let sender = self
            .user_request_senders
            .lock()
            .unwrap()
            .remove(&request_id);
        let Some(sender) = sender else {
            warn!("No sender found for user request {}", request_id);
            return;
        };

        if sender.send(UserResponse::Vault(vault_id)).is_err() {
            warn!("Failed to send response for vault request {}", request_id);
        }
    }

    pub fn deny_user_request(&self, request_id: Uuid) {
        //? Drop the sender to cancel the request
        self.user_request_senders
            .lock()
            .unwrap()
            .remove(&request_id);
    }

    /// ? Helper to get the plugin or return an RpcError if not found
    fn get_entity_plugin_error(
        &self,
        entity_id: impl Into<EntityId>,
    ) -> Result<Arc<Plugin>, RpcError> {
        let entity_id = entity_id.into();
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

    fn notify_observers(&self) {
        let observers = self.observers.lock().unwrap();
        for observer in observers.iter() {
            let _ = observer.unbounded_send(());
        }
    }

    pub fn log_event(&self, event: String) {
        let mut log = self.event_log.lock().unwrap();
        log.push(event);
        self.notify_observers();
    }
}

impl Host {
    pub async fn ping(&self, _plugin_id: &PluginId, _params: ()) -> Result<String, RpcError> {
        Ok("Pong from host".to_string())
    }

    pub async fn register_entity(
        &self,
        plugin_id: &PluginId,
        domain: Domain,
    ) -> Result<EntityId, RpcError> {
        let entity_id: EntityId = match domain {
            Domain::EthProvider => EthProviderId::new().into(),
            Domain::Page => PageId::new().into(),
            Domain::Vault => VaultId::new().into(),
            Domain::Coordinator => CoordinatorId::new().into(),
        };

        let mut entities = self.entities.lock().unwrap();
        entities.insert(entity_id, *plugin_id);

        self.notify_observers();
        Ok(entity_id)
    }

    pub async fn request_eth_provider(
        &self,
        plugin_id: &PluginId,
        chain_id: caip::ChainId,
    ) -> Result<Option<EthProviderId>, RpcError> {
        let request_id = Uuid::new_v4();
        let (sender, receiver) = oneshot::channel();

        info!(
            "Created eth provider selection request {} for chain_id: {}",
            request_id, chain_id
        );

        let user_request = UserRequest::EthProviderSelection {
            id: request_id,
            plugin_id: *plugin_id,
            chain_id,
        };

        self.user_requests.lock().unwrap().push(user_request);
        self.user_request_senders
            .lock()
            .unwrap()
            .insert(request_id, sender);

        self.notify_observers();
        let resp = receiver.await;

        // Remove the request from the list
        self.user_requests.lock().unwrap().retain(|req| match req {
            UserRequest::EthProviderSelection { id, .. } => *id != request_id,
            UserRequest::VaultSelection { .. } => true,
        });

        match resp {
            Ok(UserResponse::EthProvider(selected_provider)) => {
                info!("User selected provider: {:?}", selected_provider);
                self.notify_observers();
                Ok(Some(selected_provider))
            }
            Ok(_) => {
                warn!("Unexpected response type for EthProvider request");
                self.notify_observers();
                Err(RpcError::InternalError)
            }
            Err(_) => {
                warn!("User request cancelled - receiver dropped");
                self.notify_observers();
                Err(RpcError::InternalError)
            }
        }
    }

    pub async fn request_vault(
        &self,
        plugin_id: &PluginId,
        _params: (),
    ) -> Result<Option<VaultId>, RpcError> {
        let request_id = Uuid::new_v4();
        let (sender, receiver) = oneshot::channel();

        info!(
            "Created vault selection request {} for plugin: {}",
            request_id, plugin_id
        );

        let user_request = UserRequest::VaultSelection {
            id: request_id,
            plugin_id: *plugin_id,
        };

        self.user_requests.lock().unwrap().push(user_request);
        self.user_request_senders
            .lock()
            .unwrap()
            .insert(request_id, sender);

        self.notify_observers();
        let resp = receiver.await;

        // Remove the request from the list
        self.user_requests.lock().unwrap().retain(|req| match req {
            UserRequest::VaultSelection { id, .. } => *id != request_id,
            UserRequest::EthProviderSelection { .. } => true,
        });

        match resp {
            Ok(UserResponse::Vault(selected_vault)) => {
                info!("User selected vault: {:?}", selected_vault);
                self.notify_observers();
                Ok(Some(selected_vault))
            }
            Ok(_) => {
                warn!("Unexpected response type for Vault request");
                self.notify_observers();
                Err(RpcError::InternalError)
            }
            Err(_) => {
                warn!("User request cancelled - receiver dropped");
                self.notify_observers();
                Err(RpcError::InternalError)
            }
        }
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
        // TODO: Handle errors properly
        let resp = request.send().await.unwrap();
        info!("Received response: {:?}", resp);
        let bytes = resp.bytes().await.unwrap();
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
        self.state.lock().unwrap().insert(*plugin_id, state_data);
        Ok(())
    }

    pub async fn set_interface(
        &self,
        _plugin_id: &PluginId,
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

        let balance = vault::GetAssets.call(plugin, vault_id).await.map_err(|e| {
            warn!("Error calling BalanceOf: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(balance)
    }

    pub async fn vault_withdraw(
        &self,
        params: (VaultId, AccountId, AssetId, U256),
    ) -> Result<(), RpcError> {
        let (vault_id, to, asset, amount) = params;
        let plugin = self.get_entity_plugin_error(vault_id)?;

        vault::Withdraw
            .call(plugin, (vault_id, to, asset, amount))
            .await
            .map_err(|e| {
                warn!("Error calling Transfer: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(())
    }

    pub async fn vault_get_deposit_address(
        &self,
        params: (VaultId, AssetId),
    ) -> Result<AccountId, RpcError> {
        let (vault_id, asset) = params;
        let plugin = self.get_entity_plugin_error(vault_id)?;

        let result = vault::GetDepositAddress
            .call(plugin, (vault_id, asset))
            .await
            .map_err(|e| {
                warn!("Error calling GetReceiptAddress: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(result)
    }

    // pub async fn vault_on_deposit(
    //     &self,
    //     params: <vault::OnDeposit as RpcMethod>::Params,
    // ) -> Result<(), RpcError> {
    //     let plugin = self.get_entity_plugin_error(params.0)?;

    //     vault::OnDeposit.call(plugin, params).await.map_err(|e| {
    //         warn!("Error calling OnReceive: {:?}", e);
    //         e.as_rpc_code()
    //     })?;
    //     Ok(())
    // }

    pub async fn page_on_load(&self, page_id: PageId) -> Result<(), RpcError> {
        let plugin = self.get_entity_plugin_error(page_id)?;

        page::OnLoad.call(plugin, page_id).await.map_err(|e| {
            warn!("Error calling OnPageLoad: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(())
    }

    pub async fn page_on_update(&self, params: (PageId, page::PageEvent)) -> Result<(), RpcError> {
        let (page_id, event) = params;
        let plugin = self.get_entity_plugin_error(page_id)?;

        page::OnUpdate
            .call(plugin, (page_id, event))
            .await
            .map_err(|e| {
                warn!("Error calling OnPageUpdate: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(())
    }

    pub async fn eth_provider_chain_id(
        &self,
        provider_id: EthProviderId,
    ) -> Result<U256, RpcError> {
        let plugin = self.get_entity_plugin_error(provider_id)?;

        let chain_id = eth::ChainId.call(plugin, provider_id).await.map_err(|e| {
            warn!("Error calling ChainId: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(chain_id)
    }

    pub async fn eth_provider_block_number(
        &self,
        provider_id: EthProviderId,
    ) -> Result<u64, RpcError> {
        let plugin = self.get_entity_plugin_error(provider_id)?;

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
        let plugin = self.get_entity_plugin_error(params.0)?;

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
        let plugin = self.get_entity_plugin_error(params.0)?;

        let resp = eth::GetBalance.call(plugin, params).await.map_err(|e| {
            warn!("Error calling GetBalance: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(resp)
    }

    pub async fn eth_provider_gas_price(
        &self,
        provider_id: EthProviderId,
    ) -> Result<u128, RpcError> {
        let plugin = self.get_entity_plugin_error(provider_id)?;

        let gas_price = eth::GasPrice.call(plugin, provider_id).await.map_err(|e| {
            warn!("Error calling GasPrice: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(gas_price)
    }

    pub async fn eth_transaction_count(
        &self,
        params: <eth::GetTransactionCount as RpcMethod>::Params,
    ) -> Result<<eth::GetTransactionCount as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let resp = eth::GetTransactionCount
            .call(plugin, params)
            .await
            .map_err(|e| {
                warn!("Error calling GetTransactionCount: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(resp)
    }

    pub async fn eth_send_raw_transaction(
        &self,
        params: <eth::SendRawTransaction as RpcMethod>::Params,
    ) -> Result<<eth::SendRawTransaction as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let tx_hash = eth::SendRawTransaction
            .call(plugin, params)
            .await
            .map_err(|e| {
                warn!("Error calling SendRawTransaction: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(tx_hash)
    }

    pub async fn eth_estimate_gas(
        &self,
        params: <eth::EstimateGas as RpcMethod>::Params,
    ) -> Result<<eth::EstimateGas as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let gas_estimate = eth::EstimateGas.call(plugin, params).await.map_err(|e| {
            warn!("Error calling EstimateGas: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(gas_estimate)
    }

    pub async fn eth_get_transaction_receipt(
        &self,
        params: <eth::GetTransactionReceipt as RpcMethod>::Params,
    ) -> Result<<eth::GetTransactionReceipt as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let receipt = eth::GetTransactionReceipt
            .call(plugin, params)
            .await
            .map_err(|e| {
                warn!("Error calling GetTransactionReceipt: {:?}", e);
                e.as_rpc_code()
            })?;
        Ok(receipt)
    }

    pub async fn eth_get_block(
        &self,
        params: <eth::GetBlock as RpcMethod>::Params,
    ) -> Result<<eth::GetBlock as RpcMethod>::Output, RpcError> {
        let plugin = self.get_entity_plugin_error(params.0)?;

        let block = eth::GetBlock.call(plugin, params).await.map_err(|e| {
            warn!("Error calling GetBlock: {:?}", e);
            e.as_rpc_code()
        })?;
        Ok(block)
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
impl_host_rpc!(Host, host::Fetch, fetch);
impl_host_rpc!(Host, host::GetState, get_state);
impl_host_rpc!(Host, host::SetState, set_state);
impl_host_rpc!(Host, host::SetPage, set_interface);
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
