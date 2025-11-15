pub mod eth_provider;

use dioxus::prelude::*;
use host::host::UserRequest;

use crate::components::user_requests::eth_provider::EthProviderSelectionComponent;

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
    }
}
