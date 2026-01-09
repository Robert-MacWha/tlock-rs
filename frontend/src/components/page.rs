use dioxus::prelude::*;
use tlock_hdk::tlock_api::{entities::PageId, page::PageEvent};

use crate::{
    components::component::RenderComponent,
    contexts::{host::HostContext, toast::ToastContext},
};

#[component]
pub fn Page(id: PageId) -> Element {
    let mut ctx: HostContext = use_context();
    let toast: ToastContext = use_context();

    // Initial load, fetch the page via `OnPageLoad`
    // TODO: Cache page and only reload if necessary
    use_effect(move || {
        spawn(async move {
            if let Err(err) = ctx.page_on_load(id).await {
                info!("OnPageLoad error: {}", err);
                toast.push(
                    format!("Error loading page: {}", err),
                    crate::contexts::toast::ToastKind::Error,
                );
            }
        });
    });

    let on_component_event = use_callback(move |event: PageEvent| {
        spawn(async move {
            match ctx.page_on_update(id, event).await {
                Ok(()) => info!("OnPageUpdate success"),
                Err(err) => {
                    info!("OnPageUpdate error: {}", err);
                    toast.push(
                        format!("Error updating page: {}", err),
                        crate::contexts::toast::ToastKind::Error,
                    );
                }
            }
        });
    });

    let Some(component) = ctx.interface(id) else {
        return rsx! { "Page Uninitialized" };
    };

    rsx!(RenderComponent {
        component,
        on_event: on_component_event
    })
}
