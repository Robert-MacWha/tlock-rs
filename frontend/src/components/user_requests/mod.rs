pub mod eth_provider;
pub mod vault;

use dioxus::prelude::*;
use host::host::UserRequest;

use crate::components::user_requests::{
    eth_provider::EthProviderSelectionComponent, vault::VaultSelectionComponent,
};

#[component]
pub fn UserRequestComponent(request: UserRequest) -> Element {
    match request {
        UserRequest::EthProviderSelection { .. } => {
            rsx! {
                EthProviderSelectionComponent {
                    request: request,
                }
            }
        }
        UserRequest::VaultSelection { .. } => {
            rsx! {
                VaultSelectionComponent {
                    request: request,
                }
            }
        }
    }
}
