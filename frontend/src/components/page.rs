use dioxus::prelude::*;
use tlock_hdk::tlock_api::{entities::PageId, page::PageEvent};

use crate::{components::component::RenderComponent, contexts::host::HostContext};

#[derive(Clone, PartialEq, Props)]
pub struct PageProps {
    pub id: PageId,
}

#[component]
pub fn Page(props: PageProps) -> Element {
    let state = use_context::<HostContext>();
    let page_id = props.id;

    // Initial load, fetch the page via `OnPageLoad`
    use_resource({
        let state = state.clone();
        move || {
            let state = state.clone();
            async move {
                match state.host.page_on_load(page_id).await {
                    Ok(()) => info!("OnPageLoad success"),
                    Err(err) => info!("OnPageLoad error: {err}"),
                }
            }
        }
    });

    let on_component_event = use_callback({
        let state = state.clone();

        move |event: PageEvent| {
            let state = state.clone();
            spawn(async move {
                match state.host.page_on_update((page_id, event)).await {
                    Ok(()) => info!("OnPageUpdate success"),
                    Err(err) => info!("OnPageUpdate error: {err}"),
                }
            });
        }
    });

    rsx!(
        div {
            p {
                "Page Component"
                ul {
                    li { "ID: {props.id}" }
                    li { "Component: ", {
                        let interface = state.interfaces.read().get(&page_id).cloned();
                        match interface {
                            Some(component) => rsx! { RenderComponent { component: component, on_event: on_component_event } },
                            None => rsx! { "No component set for this interface." }
                        }}
                    }
                }
            }
        }
    )
}
