use dioxus::{logger::tracing::info, prelude::*};
use host::host::UserRequest;
use tlock_hdk::tlock_api::entities::VaultId;

use crate::contexts::host::HostContext;

#[component]
pub fn VaultSelectionComponent(request: UserRequest) -> Element {
    let state = use_context::<HostContext>();

    // Get available vaults
    let vaults: Vec<VaultId> = {
        let entities = state.entities.read();
        entities
            .iter()
            .filter_map(|entity_id| match entity_id {
                tlock_hdk::tlock_api::entities::EntityId::Vault(vault_id) => Some(*vault_id),
                _ => None,
            })
            .collect()
    };

    let (request_id, plugin_id) = match request {
        UserRequest::VaultSelection { id, plugin_id } => (id, plugin_id),
        UserRequest::EthProviderSelection { .. } => {
            panic!("EthProviderSelection request passed to Vault component")
        }
    };

    let handle_select_vault = move |vault_id: VaultId| {
        move |_| {
            info!("User selected vault: {}", vault_id);
            consume_context::<HostContext>()
                .host
                .read()
                .resolve_vault_request(request_id, vault_id);
        }
    };

    let handle_deny = move |_| {
        info!("User denied vault selection");
        consume_context::<HostContext>()
            .host
            .read()
            .deny_user_request(request_id);
    };

    rsx! {
        div {
            p {
                "Vault Selection"
                ul {
                    li { "Plugin: {plugin_id}" }
                    li {
                        "Available vaults:"
                        ul {
                            for vault_id in vaults {
                                li {
                                    key: "{vault_id}",
                                    button {
                                        onclick: handle_select_vault(vault_id),
                                        "Select Vault {vault_id}"
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
