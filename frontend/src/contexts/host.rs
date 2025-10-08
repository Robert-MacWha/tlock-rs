use std::sync::Arc;

use dioxus::{
    hooks::use_signal,
    signals::{Signal, Writable},
};
use host::host::Host;
use tlock_hdk::{
    tlock_api::entities::EntityId,
    wasmi_hdk::plugin::{PluginError, PluginId},
};

#[derive(Clone)]
pub struct HostContext {
    pub host: Arc<Host>,
    pub plugins: Signal<Vec<PluginId>>,
    pub entities: Signal<Vec<EntityId>>,
}

impl HostContext {
    pub fn new(host: Arc<Host>) -> Self {
        Self {
            host,
            plugins: use_signal(|| Vec::new()),
            entities: use_signal(|| Vec::new()),
        }
    }

    pub async fn load_plugin(
        &mut self,
        wasm_bytes: &[u8],
        name: &str,
    ) -> Result<PluginId, PluginError> {
        let id = self.host.load_plugin(wasm_bytes, name).await?;

        let mut plugins = self.plugins.write();
        plugins.push(id.clone());

        let plugin_entities = self.host.get_entities();
        self.entities.set(plugin_entities);

        Ok(id)
    }
}
