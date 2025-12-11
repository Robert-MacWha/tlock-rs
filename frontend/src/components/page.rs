use dioxus::prelude::*;
use tlock_hdk::tlock_api::{entities::PageId, page::PageEvent};

use crate::{components::component::RenderComponent, contexts::host::HostContext};

#[derive(Clone, PartialEq, Props)]
pub struct PageProps {
    pub id: PageId,
}

#[component]
pub fn Page(props: PageProps) -> Element {
    let page_id = props.id;

    // Initial load, fetch the page via `OnPageLoad`
    use_resource(move || async move {
        match consume_context::<HostContext>()
            .host
            .read()
            .page_on_load(page_id)
            .await
        {
            Ok(()) => info!("OnPageLoad success"),
            Err(err) => info!("OnPageLoad error: {err}"),
        }
    });

    let on_component_event = use_callback(move |event: PageEvent| {
        spawn(async move {
            match consume_context::<HostContext>()
                .host
                .read()
                .page_on_update((page_id, event))
                .await
            {
                Ok(()) => info!("OnPageUpdate success"),
                Err(err) => info!("OnPageUpdate error: {err}"),
            }
        });
    });

    rsx!(
        div {
            p {
                "Page Component"
                ul {
                    li { "ID: {props.id}" }
                    li { "Component: ", {
                        let interface = consume_context::<HostContext>().interfaces.read().get(&page_id).cloned();
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
