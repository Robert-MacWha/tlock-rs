use dioxus::prelude::*;
use dioxus_logger::tracing::info;
use tlock_hdk::tlock_api::{entities::PageId, page::PageEvent};

use crate::{components::component::RenderComponent, contexts::host::HostContext};

#[derive(Clone, PartialEq, Props)]
pub struct PageProps {
    pub id: PageId,
}

#[component]
pub fn Page(props: PageProps) -> Element {
    let interface_id = use_hook(|| {
        let dest = &mut [0u8; 4];
        let _ = getrandom::getrandom(dest);
        return u32::from_le_bytes(*dest);
    });

    let state = use_context::<HostContext>();
    let entity_plugin = state.host.get_entity_plugin(&props.id.as_entity_id());
    let entity_plugin = match entity_plugin {
        Some(id) => id,
        None => return rsx! { div { "Page component - ID: {props.id}, Plugin: Unknown" } },
    };

    // Initial load, fetch the page via `OnPageLoad`
    let _ = use_resource({
        let state = state.clone();
        let plugin_id = entity_plugin.id().clone();
        move || {
            let mut state = state.clone();
            let plugin_id = plugin_id.clone();

            async move {
                match state.host.page_on_load(&plugin_id, interface_id).await {
                    Ok(()) => info!("OnPageLoad success"),
                    Err(err) => info!("OnPageLoad error: {err}"),
                }
                state.reload_state();
            }
        }
    });

    let on_component_event = use_callback({
        let state = state.clone();
        let plugin_id = entity_plugin.id().clone();
        move |event: PageEvent| {
            let mut state = state.clone();
            let plugin_id = plugin_id.clone();

            spawn(async move {
                match state
                    .host
                    .page_on_update(&plugin_id, interface_id, event)
                    .await
                {
                    Ok(()) => info!("OnPageUpdate success"),
                    Err(err) => info!("OnPageUpdate error: {err}"),
                }
                state.reload_state();
            });
        }
    });

    rsx!(
        div {
            p {
                "Page Component"
                ul {
                    li { "ID: {props.id}" }
                    li { "Plugin: {entity_plugin.name()} ({entity_plugin.id()})" }
                    li { "Interface ID: {interface_id}" }
                    li { "Component: ", {
                        let interface = state.interfaces.read().get(&interface_id).cloned();
                        match interface {
                            Some(component) => rsx! { RenderComponent { component: component, on_event: on_component_event } },
                            None => rsx! { "No component set for this interface." }
                        }}
                    }
                }
            }
        }
    )

    // let mut ping_resp = use_signal(|| "".to_string());
    // let mut balance_of_resp = use_signal(|| "".to_string());

    // let handle_ping = {
    //     let state = state.clone();
    //     let plugin_id = entity_plugin.id().clone();

    //     move |_| {
    //         spawn({
    //             let state = state.clone();
    //             let plugin_id = plugin_id.clone();
    //             async move {
    //                 info!("Ping plugin {plugin_id}");

    //                 let response = match state.host.ping_plugin(&plugin_id).await {
    //                     Ok(resp) => format!("Ping response: {resp}"),
    //                     Err(err) => format!("Ping error: {err}"),
    //                 };
    //                 ping_resp.set(response);
    //             }
    //         });
    //     }
    // };

    // let handle_balance_of = {
    //     let state = state.clone();
    //     let vault_id = props.id.clone();

    //     move |_| {
    //         balance_of_resp.set("...".into());
    //         spawn({
    //             let state = state.clone();
    //             let vault_id = vault_id.clone();
    //             async move {
    //                 info!("BalanceOf for vault {vault_id}");

    //                 let response = match state.host.balance_of(vault_id).await {
    //                     Ok(balances) => {
    //                         let mut resp = String::new();
    //                         for (asset, amount) in balances {
    //                             resp.push_str(&format!("Asset: {:?}, Amount: {}\n", asset, amount));
    //                         }
    //                         resp
    //                     }
    //                     Err(err) => format!("BalanceOf error: {err}"),
    //                 };
    //                 balance_of_resp.set(response);
    //             }
    //         });
    //     }
    // };

    // rsx! {
    //     div {
    //         p {
    //             "Vault Component"
    //             ul {
    //                 li { "ID: {props.id}" }
    //                 li { "Plugin: {entity_plugin.name()} ({entity_plugin.id()})" }
    //                 li {
    //                     button {
    //                         onclick: handle_ping,
    //                         "Ping Plugin"
    //                     }
    //                     "{ping_resp}"
    //                 }
    //                 li {
    //                     button {
    //                         onclick: handle_balance_of,
    //                         "Get Balance"
    //                     }
    //                     "{balance_of_resp}"
    //                 }
    //             }
    //         }
    //     }
    // }
}
