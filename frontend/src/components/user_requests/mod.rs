use std::fmt::Debug;

use dioxus::prelude::*;
use host::host::UserRequest;
use tlock_hdk::tlock_api::entities::EntityId;

use crate::contexts::host::HostContext;

#[component]
pub fn UserRequestComponent(request: UserRequest) -> Element {
    let mut ctx: HostContext = use_context();

    // Helper to get common data
    let plugin_id = request.plugin_id();
    let plugin_name = ctx
        .plugin(plugin_id)
        .map(|p| p.name().to_string())
        .unwrap_or("Unknown Plugin".to_string());

    match request {
        UserRequest::EthProviderSelection { id, chain_id, .. } => rsx! {
            SelectionWrapper { title: "Ethereum Provider ({chain_id})", plugin_name,
                EntitySelection {
                    filter_map: |eid| match eid {
                        EntityId::EthProvider(i) => Some(i),
                        _ => None,
                    },
                    on_deny: move |_| ctx.deny_user_request(id),
                    on_select: move |selected_id| ctx.resolve_eth_provider_request(id, selected_id),
                }
            }
        },
        UserRequest::VaultSelection { id, .. } => rsx! {
            SelectionWrapper { title: "Vault", plugin_name,
                EntitySelection {
                    filter_map: |eid| match eid {
                        EntityId::Vault(i) => Some(i),
                        _ => None,
                    },
                    on_deny: move |_| ctx.deny_user_request(id),
                    on_select: move |selected_id| ctx.resolve_vault_request(id, selected_id),
                }
            }
        },
        UserRequest::CoordinatorSelection { id, .. } => rsx! {
            SelectionWrapper { title: "Coordinator", plugin_name,
                EntitySelection {
                    filter_map: |eid| match eid {
                        EntityId::Coordinator(i) => Some(i),
                        _ => None,
                    },
                    on_deny: move |_| ctx.deny_user_request(id),
                    on_select: move |selected_id| ctx.resolve_coordinator_request(id, selected_id),
                }
            }
        },
    }
}

#[component]
fn SelectionWrapper(title: String, plugin_name: String, children: Element) -> Element {
    rsx! {
        div { class: "menu w-full",
            h3 { class: "menu-title",
                "{title} requested by "
                span { class: "font-bold", "{plugin_name}" }
            }
            {children}
        }
    }
}

#[component]
fn EntitySelection<T>(
    filter_map: Callback<EntityId, Option<T>>,
    on_select: EventHandler<T>,
    on_deny: EventHandler<()>,
) -> Element
where
    T: PartialEq + Debug + Copy + 'static,
{
    let ctx: HostContext = use_context();
    let entities = ctx.entity_ids();
    let entities: Vec<(EntityId, T)> = entities
        .iter()
        .filter_map(|entity_id| filter_map.call(*entity_id).map(|t| (*entity_id, t)))
        .collect();

    rsx!(
        ul {
            for entity in entities {
                li { key: "entity-{entity:?}",
                    SelectableEntity {
                        id: entity.0,
                        entity: entity.1,
                        on_select: on_select.clone(),
                    }
                }
            }
            div { class: "divider" }
            li {
                button { class: "text-error", onclick: move |_| on_deny.call(()), "Deny Request" }
            }
        }
    )
}

#[component]
fn SelectableEntity<T>(id: EntityId, entity: T, on_select: EventHandler<T>) -> Element
where
    T: PartialEq + Debug + Copy + 'static,
{
    let ctx: HostContext = use_context();
    let entity_plugin = ctx.entity_plugin(id);
    let plugin_name = entity_plugin
        .as_ref()
        .map(|p| p.name())
        .unwrap_or("Unknown Plugin");

    rsx!(
        button { onclick: move |_| on_select.call(entity), "{id} (plugin: {plugin_name})" }
    )
}
