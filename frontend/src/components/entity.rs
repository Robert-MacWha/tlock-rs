use dioxus::prelude::*;
use tlock_hdk::tlock_api::entities::EntityId;

use crate::components::vault::Vault;

#[derive(Clone, PartialEq, Props)]
pub struct EntityProps {
    pub id: EntityId,
}

#[component]
pub fn Entity(props: EntityProps) -> Element {
    match props.id {
        EntityId::Vault(id) => rsx! { Vault { id: id } },
        _ => rsx! { div { "Unknown entity type" } },
    }
}
