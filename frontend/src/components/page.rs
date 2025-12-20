use dioxus::prelude::*;
use tlock_hdk::tlock_api::{entities::PageId, page::PageEvent};

use crate::{components::component::RenderComponent, contexts::host::HostContext};

#[component]
pub fn Page(id: PageId) -> Element {
    // Initial load, fetch the page via `OnPageLoad`
    use_resource(move || async move {
        match consume_context::<HostContext>()
            .host
            .read()
            .page_on_load(id)
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
                .page_on_update((id, event))
                .await
            {
                Ok(()) => info!("OnPageUpdate success"),
                Err(err) => info!("OnPageUpdate error: {err}"),
            }
        });
    });

    rsx!(
        p { "ID: {id}" }
        {
            let interface = consume_context::<HostContext>().interfaces.read().get(&id).cloned();
            match interface {
                Some(component) => rsx! { RenderComponent { component: component, on_event: on_component_event } },
                None => rsx! { "No component set for this interface." }
        }}
    )
}
