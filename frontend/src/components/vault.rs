use dioxus::prelude::*;
use dioxus_logger::tracing::info;
use tlock_hdk::tlock_api::entities::VaultId;

use crate::contexts::host::HostContext;

#[derive(Clone, PartialEq, Props)]
pub struct VaultProps {
    pub id: VaultId,
}

#[component]
pub fn Vault(props: VaultProps) -> Element {
    let state = use_context::<HostContext>();
    let entity_plugin = state.host.get_entity_plugin(&props.id.as_entity_id());
    let entity_plugin = match entity_plugin {
        Some(id) => id,
        None => return rsx! { div { "Vault component - ID: {props.id}, Plugin: Unknown" } },
    };

    let mut ping_resp = use_signal(|| "".to_string());
    let mut balance_of_resp = use_signal(|| "".to_string());

    let handle_ping = {
        let state = state.clone();
        let plugin_id = entity_plugin.id().clone();

        move |_| {
            spawn({
                let state = state.clone();
                let plugin_id = plugin_id.clone();
                async move {
                    info!("Ping plugin {plugin_id}");

                    // Test artificial work
                    ping_resp.set("Doing fake work...".into());
                    for _ in 0..20 {
                        let resp = reqwest::get("https://httpbin.org/get").await;
                        info!("Ping sub-request result: {resp:?}");
                    }

                    let response = match state.host.ping_plugin(&plugin_id).await {
                        Ok(resp) => format!("Ping response: {resp}"),
                        Err(err) => format!("Ping error: {err}"),
                    };
                    ping_resp.set(response);
                }
            });
        }
    };

    let handle_balance_of = {
        let state = state.clone();
        let vault_id = props.id.clone();

        move |_| {
            spawn({
                balance_of_resp.set("...".into());
                let state = state.clone();
                let vault_id = vault_id.clone();
                async move {
                    info!("BalanceOf for vault {vault_id}");

                    let response = match state.host.vault_get_assets(vault_id).await {
                        Ok(balances) => {
                            let mut resp = String::new();
                            for (asset, amount) in balances {
                                resp.push_str(&format!("Asset: {:?}, Amount: {}\n", asset, amount));
                            }
                            resp
                        }
                        Err(err) => format!("BalanceOf error: {err}"),
                    };
                    balance_of_resp.set(response);
                }
            });
        }
    };

    let handle_transfer = {
        let state = state.clone();
        let vault_id = props.id.clone();

        move |_| {
            spawn({
                let _state = state.clone();
                let vault_id = vault_id.clone();
                async move {
                    info!("TODO: Transfer from vault {vault_id}");
                }
            });
        }
    };

    rsx! {
        div {
            p {
                "Vault Component"
                ul {
                    li { "ID: {props.id}" }
                    li { "Plugin: {entity_plugin.name()} ({entity_plugin.id()})" }
                    li {
                        button {
                            onclick: handle_ping,
                            "Ping Plugin"
                        }
                        "{ping_resp}"
                    }
                    li {
                        button {
                            onclick: handle_balance_of,
                            "Get Balance"
                        }
                        "{balance_of_resp}"
                    }
                    li {
                        button {
                            onclick: handle_transfer,
                            "Transfer"
                        }
                    }
                }
            }
        }
    }
}
