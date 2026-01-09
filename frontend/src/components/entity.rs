use dioxus::prelude::*;
use tlock_hdk::tlock_api::entities::EntityId;

use crate::contexts::host::HostContext;

#[component]
pub fn Entity(id: EntityId) -> Element {
    let ctx: HostContext = use_context();
    let entity_plugin = ctx.entity_plugin(id);
    let plugin_name = entity_plugin
        .as_ref()
        .map(|p| p.name())
        .unwrap_or("Unknown Plugin");

    rsx! {
        div { "{id} (provided by {plugin_name})" }
    }
}
