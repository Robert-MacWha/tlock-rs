use dioxus::{logger::tracing::info, prelude::*};
use host::host::UserRequest;
use tlock_hdk::tlock_api::entities::EthProviderId;

use crate::contexts::host::HostContext;

#[component]
pub fn EthProviderSelectionComponent(request: UserRequest) -> Element {
    let state = use_context::<HostContext>();

    // Get available eth providers
    let eth_providers: Vec<EthProviderId> = {
        let entities = state.entities.read();
        entities
            .iter()
            .filter_map(|entity_id| match entity_id {
                tlock_hdk::tlock_api::entities::EntityId::EthProvider(provider_id) => {
                    Some(*provider_id)
                }
                _ => None,
            })
            .collect()
    };

    let (request_id, plugin_id, chain_id) = match request {
        UserRequest::EthProviderSelection {
            id,
            plugin_id,
            chain_id,
        } => (id, plugin_id, chain_id),
        UserRequest::VaultSelection { .. } => {
            panic!("VaultSelection request passed to EthProvider component")
        }
    };

    let state_for_handlers = state.clone();
    let handle_select_provider = move |provider_id: EthProviderId| {
        let state = state_for_handlers.clone();
        move |_| {
            let state = state.clone();
            info!("User selected provider: {}", provider_id);
            state.host.resolve_user_request(request_id, provider_id);
        }
    };

    let handle_deny = {
        let state = state.clone();
        move |_| {
            let state = state.clone();
            info!("User denied provider selection");
            state.host.deny_user_request(request_id);
        }
    };

    rsx! {
        div {
            p {
                "Ethereum Provider Selection"
                ul {
                    li { "Plugin: {plugin_id}" }
                    li { "Chain: {chain_id}" }
                    li {
                        "Available providers:"
                        ul {
                            for provider_id in eth_providers {
                                li {
                                    key: "{provider_id}",
                                    button {
                                        onclick: handle_select_provider(provider_id),
                                        "Select Provider {provider_id}"
                                    }
                                }
                            }
                        }
                    }
                    li {
                        button {
                            onclick: handle_deny,
                            "Deny"
                        }
                    }
                }
            }
        }
    }
}
