use dioxus::prelude::*;
use dioxus_logger::tracing::info;
use tlock_hdk::tlock_api::entities::EthProviderId;

use crate::contexts::host::HostContext;

#[derive(Clone, PartialEq, Props)]
pub struct EthProviderProps {
    pub id: EthProviderId,
}

#[component]
pub fn EthProvider(props: EthProviderProps) -> Element {
    let state = use_context::<HostContext>();
    let eth_provider_id = props.id.clone();
    let mut block_number_resp = use_signal(|| 0u64);

    let handle_block_number = move |_| {
        let state = state.clone();

        spawn(async move {
            info!("Fetch block number for EthProvider {eth_provider_id}");
            match state.host.eth_provider_block_number(eth_provider_id).await {
                Ok(block_number) => {
                    info!("Block number: {block_number}");
                    block_number_resp.set(block_number);
                }
                Err(err) => {
                    info!("Error fetching block number: {err}");
                    block_number_resp.set(0);
                }
            }
        });
    };

    rsx! {
        div {
            p {
                "EthProvider Component"
                ul {
                    li { "ID: {props.id}" }
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
