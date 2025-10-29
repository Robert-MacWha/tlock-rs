use std::{collections::HashMap, sync::Arc};

use dioxus::{
    hooks::use_signal,
    signals::{Signal, Writable},
};
use host::host::Host;
use tlock_hdk::{
    tlock_api::{component::Component, entities::EntityId},
    wasmi_hdk::plugin::{PluginError, PluginId},
};
use tracing_log::log::info;

#[derive(Clone)]
pub struct HostContext {
    pub host: Arc<Host>,
    pub plugins: Signal<Vec<PluginId>>,
    pub entities: Signal<Vec<EntityId>>,
    pub interfaces: Signal<HashMap<u32, Component>>, // interface_id -> Component
}

impl HostContext {
    pub fn new(host: Arc<Host>) -> Self {
        Self {
            host,
            plugins: use_signal(Vec::new),
            entities: use_signal(Vec::new),
            interfaces: use_signal(HashMap::new),
        }
    }

    pub async fn load_plugin(
        &mut self,
        wasm_bytes: &[u8],
        name: &str,
    ) -> Result<PluginId, PluginError> {
        let id = self.host.load_plugin(wasm_bytes, name).await?;
        self.reload_state();
        Ok(id)
    }

    // TODO: Setup a watcher where the host notifies us of changes and we update
    // automatically.  Right now we need to manually refresh, which is both
    // inefficient and *very* error-prone.
    pub fn reload_state(&mut self) {
        info!("Reloading HostContext state");
        let plugins = self.host.get_plugins();
        self.plugins.set(plugins);

        let plugin_entities = self.host.get_entities();
        self.entities.set(plugin_entities);

        let interfaces = self.host.get_interfaces();
        self.interfaces.set(interfaces);
    }
}
