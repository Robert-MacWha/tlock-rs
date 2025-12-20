use dioxus::prelude::*;
use tlock_hdk::tlock_api::entities::EntityId;

use crate::contexts::host::HostContext;

#[component]
pub fn Entity(id: EntityId) -> Element {
    let entity_plugin = consume_context::<HostContext>()
        .host
        .read()
        .get_entity_plugin(id);
    let plugin_name = entity_plugin
        .as_ref()
        .map(|p| p.name())
        .unwrap_or("Unknown Plugin");

    rsx! {
        div { "{id} (provided by {plugin_name})" }
    }
}
