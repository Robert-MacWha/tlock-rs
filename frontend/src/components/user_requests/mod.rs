use std::fmt::Debug;

use dioxus::prelude::*;
use host::host::UserRequest;
use tlock_hdk::tlock_api::entities::EntityId;

use crate::contexts::host::HostContext;

#[component]
pub fn UserRequestComponent(request: UserRequest) -> Element {
    match request {
        UserRequest::EthProviderSelection { .. } => {
            rsx! { EthProviderSelectionComponent { request } }
        }
        UserRequest::VaultSelection { .. } => {
            rsx! { VaultSelectionComponent { request } }
        }
        UserRequest::CoordinatorSelection { .. } => {
            rsx! { CoordinatorSelectionComponent { request } }
        }
    }
}

#[component]
fn EthProviderSelectionComponent(request: UserRequest) -> Element {
    let (request_id, plugin_id, chain_id) = match request {
        UserRequest::EthProviderSelection {
            id,
            plugin_id,
            chain_id,
        } => (id, plugin_id, chain_id),
        _ => {
            panic!("EthProviderSelection request passed to wrong component")
        }
    };

    rsx!(
        div {
            p { "Ethereum Provider Selection" }
            ul {
                li { "Plugin: {plugin_id}" }
                li { "Chain ID: {chain_id}" }
                EntitySelection {
                    filter_map: move |entity_id| match entity_id {
                        EntityId::EthProvider(id) => Some(id),
                        _ => None,
                    },
                    on_select: move |id| {
                        consume_context::<HostContext>()
                            .host
                            .read()
                            .resolve_eth_provider_request(request_id, id);
                    },
                    on_deny: move || {
                        consume_context::<HostContext>()
                            .host
                            .read()
                            .deny_user_request(request_id);
                        }
                }
            }
        }
    )
}

#[component]
fn VaultSelectionComponent(request: UserRequest) -> Element {
    let (request_id, plugin_id) = match request {
        UserRequest::VaultSelection { id, plugin_id } => (id, plugin_id),
        _ => {
            panic!("VaultSelection request passed to wrong component")
        }
    };

    rsx!(
        div {
            p { "Vault Selection" }
            ul {
                li { "Plugin: {plugin_id}" }
                EntitySelection {
                    filter_map: move |entity_id| match entity_id {
                        EntityId::Vault(id) => Some(id),
                        _ => None,
                    },
                    on_select: move |id| {
                        consume_context::<HostContext>()
                            .host
                            .read()
                            .resolve_vault_request(request_id, id);
                    },
                    on_deny: move || {
                        consume_context::<HostContext>()
                            .host
                            .read()
                            .deny_user_request(request_id);
                        }
                }
            }
        }
    )
}

#[component]
fn CoordinatorSelectionComponent(request: UserRequest) -> Element {
    let (request_id, plugin_id) = match request {
        UserRequest::CoordinatorSelection { id, plugin_id } => (id, plugin_id),
        _ => {
            panic!("CoordinatorSelection request passed to wrong component")
        }
    };

    rsx!(
        div {
            p { "Coordinator Selection" }
            ul {
                li { "Plugin: {plugin_id}" }
                EntitySelection {
                    filter_map: move |entity_id| match entity_id {
                        EntityId::Coordinator(id) => Some(id),
                        _ => None,
                    },
                    on_select: move |id| {
                        consume_context::<HostContext>()
                            .host
                            .read()
                            .resolve_coordinator_request(request_id, id);
                    },
                    on_deny: move || {
                        consume_context::<HostContext>()
                            .host
                            .read()
                            .deny_user_request(request_id);
                        }
                }
            }
        }
    )
}

// TODO: Consider abstracting on_deny into EntitySelection so that each
// component doesn't have to repeat it. It should be shared across all
// components I expect.
#[component]
fn EntitySelection<T>(
    filter_map: Callback<EntityId, Option<T>>,
    on_select: EventHandler<T>,
    on_deny: EventHandler<()>,
) -> Element
where
    T: PartialEq + Debug + Copy + 'static,
{
    let entities = consume_context::<HostContext>()
        .entities
        .read()
        .iter()
        .filter_map(|entity_id| filter_map.call(*entity_id))
        .collect::<Vec<T>>();

    rsx!(
        div {
            ul {
                for entity in entities {
                    li {
                        key: "{entity:?}",
                        button {
                            onclick: move |_| on_select.call(entity),
                            "Select {entity:?}"
                        }
                    }
                }
            }
            button {
                onclick: move |_| on_deny.call(()),
                "Deny Request"
            }
         }
    )
}
