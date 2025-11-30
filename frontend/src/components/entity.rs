use dioxus::prelude::*;
use tlock_hdk::tlock_api::entities::EntityId;

use crate::components::page::Page;

#[derive(Clone, PartialEq, Props)]
pub struct EntityProps {
    pub id: EntityId,
}

#[component]
pub fn Entity(props: EntityProps) -> Element {
    match props.id {
        EntityId::Page(id) => rsx! { Page {id: id} },
        _ => rsx! { p { "Entity {props.id} cannot be rendered" } },
    }
}
