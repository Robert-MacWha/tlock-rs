use dioxus::prelude::*;
use dioxus_logger::tracing::info;
use tlock_hdk::tlock_api::entities::EthProviderId;

use crate::{components::log_fut::log_future, contexts::host::HostContext};

#[derive(Clone, PartialEq, Props)]
pub struct EthProviderProps {
    pub id: EthProviderId,
}

#[component]
pub fn EthProvider(props: EthProviderProps) -> Element {
    let state = use_context::<HostContext>();
    let entity_plugin = state.host.get_entity_plugin(&props.id.as_entity_id());
    let entity_plugin = match entity_plugin {
        Some(plugin) => plugin,
        None => return rsx! { div { "EthProvider component - ID: {props.id}, Plugin: Unknown" } },
    };
    let id = entity_plugin.id();

    let mut block_number_resp = use_signal(|| 0u64);

    let handle_block_number = move |_| {
        let state = state.clone();
        let id = id.clone();

        spawn(log_future("eth_provider_block_number", async move {
            info!("Fetch block number for EthProvider {id}");
            match state.host.eth_provider_block_number(&id).await {
                Ok(block_number) => {
                    info!("Block number: {block_number}");
                    block_number_resp.set(block_number);
                }
                Err(err) => {
                    info!("Error fetching block number: {err}");
                    block_number_resp.set(0);
                }
            }
        }));
    };

    rsx! {
        div {
            p {
                "EthProvider Component"
                ul {
                    li { "ID: {props.id}" }
                    li { "Plugin: {entity_plugin.name()} ({entity_plugin.id()})" }
                    li {
                        button {
                            onclick: handle_block_number,
                            "Get Block Number"
                        }
                        "{block_number_resp}"
                    }
                }
            }
        }
    }
}
