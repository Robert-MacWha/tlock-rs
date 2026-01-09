use std::sync::Arc;

use dioxus::{
    hooks::{UnboundedReceiver, use_coroutine, use_coroutine_handle, use_signal},
    signals::{ReadableExt, Signal, WritableExt},
};
use futures::StreamExt;
use host::{
    host::{Event, Host, PluginError, UserRequest},
    host_state::PluginSource,
};
use tlock_hdk::{
    tlock_api::{
        component::Component,
        entities::{CoordinatorId, EntityId, EthProviderId, PageId, VaultId},
        page::PageEvent,
    },
    wasmi_plugin_hdk::{plugin::Plugin, plugin_id::PluginId},
    wasmi_plugin_pdk::rpc_message::RpcError,
};
use uuid::Uuid;

#[derive(Copy, Clone)]
pub struct HostContext {
    host: Signal<Arc<Host>>,
    revision: Signal<usize>,
}

impl HostContext {
    pub fn new(host: Arc<Host>) -> Self {
        let host_sig = use_signal(|| host);
        let mut revision = use_signal(|| 0);

        use_coroutine(move |mut rx: UnboundedReceiver<()>| async move {
            while let Some(_) = rx.next().await {
                revision += 1;
            }
        });

        let tx = use_coroutine_handle::<()>().tx();
        host_sig.read().subscribe(tx);

        Self {
            host: host_sig,
            revision,
        }
    }

    fn notify(&mut self) {
        self.revision += 1;
    }

    //? --- Reactive Getters ---
    pub fn plugin_ids(&self) -> Vec<PluginId> {
        let _ = self.revision.read();
        self.host.read().get_plugins()
    }

    pub fn plugin(&self, id: PluginId) -> Option<Plugin> {
        let _ = self.revision.read();
        self.host.read().get_plugin(&id)
    }

    pub fn plugins(&self) -> Vec<Plugin> {
        let plugin_ids = self.plugin_ids();
        plugin_ids
            .iter()
            .filter_map(|id| self.host.read().get_plugin(id))
            .collect()
    }

    pub fn entity_ids(&self) -> Vec<EntityId> {
        let _ = self.revision.read();
        self.host.read().get_entities()
    }

    pub fn entity_plugin(&self, entity_id: EntityId) -> Option<Plugin> {
        let _ = self.revision.read();
        self.host.read().get_entity_plugin(entity_id)
    }

    pub fn page_ids(&self) -> Vec<PageId> {
        let entity_ids = self.entity_ids();
        entity_ids
            .iter()
            .filter_map(|id| match id {
                EntityId::Page(page_id) => Some(*page_id),
                _ => None,
            })
            .collect()
    }

    pub fn interface(&self, page_id: PageId) -> Option<Component> {
        let _ = self.revision.read();
        self.host.read().get_interface(page_id)
    }

    pub fn requests(&self) -> Vec<UserRequest> {
        let _ = self.revision.read();
        self.host.read().get_user_requests()
    }

    pub fn events(&self) -> Vec<Event> {
        let _ = self.revision.read();
        self.host.read().get_events()
    }

    //? --- Actions ---
    pub fn set_host(&mut self, host: Arc<Host>) {
        self.host.set(host);
        self.notify();
    }

    pub async fn new_plugin(
        &mut self,
        source: PluginSource,
        name: &str,
    ) -> Result<PluginId, PluginError> {
        let host = self.host.read().clone();
        let id = host.new_plugin(source, name).await?;
        self.notify();
        Ok(id)
    }

    pub async fn page_on_load(&mut self, page_id: PageId) -> Result<(), RpcError> {
        let host = self.host.read().clone();
        host.page_on_load(page_id).await?;
        self.notify();
        Ok(())
    }

    pub async fn page_on_update(
        &mut self,
        page_id: PageId,
        event: PageEvent,
    ) -> Result<(), RpcError> {
        let host = self.host.read().clone();
        host.page_on_update((page_id, event)).await?;
        self.notify();
        Ok(())
    }

    pub fn resolve_eth_provider_request(&mut self, request_id: Uuid, provider_id: EthProviderId) {
        let host = self.host.read().clone();
        host.resolve_eth_provider_request(request_id, provider_id);
        self.notify();
    }

    pub fn resolve_vault_request(&mut self, request_id: Uuid, vault_id: VaultId) {
        let host = self.host.read().clone();
        host.resolve_vault_request(request_id, vault_id);
        self.notify();
    }

    pub fn resolve_coordinator_request(&mut self, request_id: Uuid, coordinator_id: CoordinatorId) {
        let host = self.host.read().clone();
        host.resolve_coordinator_request(request_id, coordinator_id);
        self.notify();
    }

    pub fn deny_user_request(&mut self, request_id: Uuid) {
        let host = self.host.read().clone();
        host.deny_user_request(request_id);
        self.notify();
    }
}
