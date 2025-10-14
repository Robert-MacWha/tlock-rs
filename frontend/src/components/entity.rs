use dioxus::prelude::*;
use tlock_hdk::tlock_api::entities::EntityId;

use crate::components::{page::Page, vault::Vault};

#[derive(Clone, PartialEq, Props)]
pub struct EntityProps {
    pub id: EntityId,
}

#[component]
pub fn Entity(props: EntityProps) -> Element {
    match props.id {
        EntityId::Vault(id) => rsx! { Vault { id: id } },
        EntityId::Page(id) => rsx! { Page {id: id} },
        EntityId::EthProvider(_id) => rsx! { "Eth Provider" },
    }
}
