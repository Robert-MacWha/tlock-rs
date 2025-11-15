use std::str::FromStr;

use dioxus::prelude::*;
use dioxus_logger::tracing::info;
use tlock_hdk::tlock_api::{
    alloy::primitives::{Address, U256},
    caip::{AccountId, AssetId, ChainId},
    entities::VaultId,
};

use crate::contexts::host::HostContext;

#[derive(Clone, PartialEq, Props)]
pub struct VaultProps {
    pub id: VaultId,
}

#[component]
pub fn Vault(props: VaultProps) -> Element {
    let state = use_context::<HostContext>();
    let entity_plugin = state.host.get_entity_plugin(props.id);
    let entity_plugin = match entity_plugin {
        Some(id) => id,
        None => return rsx! { div { "Vault component - ID: {props.id}, Plugin: Unknown" } },
    };

    let mut ping_resp = use_signal(|| "".to_string());
    let mut balance_of_resp = use_signal(|| "".to_string());
    let mut deposit_address_resp = use_signal(|| "".to_string());
    let mut withdraw_resp = use_signal(|| "".to_string());

    let handle_ping = {
        let state = state.clone();
        let plugin_id = entity_plugin.id().clone();

        move |_| {
            spawn({
                ping_resp.set("...".into());
                let state = state.clone();
                let plugin_id = plugin_id.clone();
                async move {
                    info!("Ping plugin {plugin_id}");

                    // Test artificial work
                    let response = match state.host.ping_plugin(&plugin_id).await {
                        Ok(resp) => format!("Ping response: {resp}"),
                        Err(err) => format!("Ping error: {err}"),
                    };
                    ping_resp.set(response);
                }
            });
        }
    };

    let handle_balance_of = {
        let state = state.clone();
        let vault_id = props.id.clone();

        move |_| {
            spawn({
                balance_of_resp.set("...".into());
                let state = state.clone();
                let vault_id = vault_id.clone();
                async move {
                    info!("BalanceOf for vault {vault_id}");

                    let response = match state.host.vault_get_assets(vault_id).await {
                        Ok(balances) => {
                            let mut resp = String::new();
                            for (asset, amount) in balances {
                                resp.push_str(&format!("Asset: {:?}, Amount: {}\n", asset, amount));
                            }
                            resp
                        }
                        Err(err) => format!("BalanceOf error: {err}"),
                    };
                    balance_of_resp.set(response);
                }
            });
        }
    };

    let handle_get_deposit_address = {
        let state = state.clone();
        let vault_id = props.id.clone();

        move |_| {
            spawn({
                let state = state.clone();
                let vault_id = vault_id.clone();
                async move {
                    info!("GetDepositAddress for vault {vault_id}");
                    let sepolia_asset_id =
                        AssetId::new(ChainId::new_evm(11155111), "slip44".into(), "60".into());

                    let response = match state
                        .host
                        .vault_get_deposit_address((vault_id, sepolia_asset_id))
                        .await
                    {
                        Ok(address) => format!("Deposit Address: {}", address),
                        Err(err) => format!("GetDepositAddress error: {err}"),
                    };
                    deposit_address_resp.set(response);
                }
            });
        }
    };

    let handle_withdraw = {
        let state = state.clone();
        let vault_id = props.id.clone();

        move |e: FormEvent| {
            spawn({
                let state = state.clone();
                let vault_id = vault_id.clone();
                let form_data = e.data();
                async move {
                    info!("Withdraw for vault {vault_id}");

                    let to_address = form_data
                        .values()
                        .get("to_address")
                        .cloned()
                        .unwrap_or_default();
                    let Ok(to_address) = Address::from_str(&to_address.as_value()) else {
                        withdraw_resp.set("Invalid to_address".into());
                        return;
                    };
                    let account_id = AccountId::new(ChainId::new_evm(11155111), to_address);
                    let amount_str = form_data
                        .values()
                        .get("amount")
                        .cloned()
                        .unwrap_or_default();
                    let Ok(amount) = U256::from_str(&amount_str.as_value()) else {
                        withdraw_resp.set("Invalid amount".into());
                        return;
                    };
                    let token = form_data
                        .values()
                        .get("token")
                        .cloned()
                        .unwrap_or_default()
                        .as_value()
                        .to_string();
                    info!(
                        "Withdraw to: {}, amount: {}, token: {}",
                        to_address, amount, token
                    );

                    let asset_id = match token.as_str() {
                        "ETH" => {
                            AssetId::new(ChainId::new_evm(11155111), "slip44".into(), "60".into())
                        }
                        "USDC" => AssetId::new(
                            ChainId::new_evm(11155111),
                            "erc20".into(),
                            "1c7d4b196cb0c7b01d743fbc6116a902379c7238".into(),
                        ),
                        _ => {
                            withdraw_resp.set("Unsupported token".into());
                            return;
                        }
                    };

                    let response = state
                        .host
                        .vault_withdraw((vault_id, account_id, asset_id, amount))
                        .await;

                    match response {
                        Ok(()) => {
                            withdraw_resp.set("Withdraw successful".into());
                        }
                        Err(err) => {
                            withdraw_resp.set(format!("Withdraw RPC error: {err}"));
                        }
                    }
                }
            });
        }
    };

    rsx! {
        div {
            p {
                "Vault Component"
                ul {
                    li { "ID: {props.id}" }
                    li { "Plugin: {entity_plugin.name()} ({entity_plugin.id()})" }
                    li {
                        button {
                            onclick: handle_ping,
                            "Ping Plugin"
                        }
                        "{ping_resp}"
                    }
                    li {
                        button {
                            onclick: handle_balance_of,
                            "Get Balance"
                        }
                        "{balance_of_resp}"
                    }
                    li {
                        button {
                            onclick: handle_get_deposit_address,
                            "Get ETH Deposit Address"
                        }
                        "{deposit_address_resp}"
                    }
                    li {
                        form {
                            onsubmit: handle_withdraw,
                            label {
                                for: "to_address",
                                "To Address:"
                            }
                            input {
                                id: "to_address",
                                name: "to_address",
                                r#type: "text",
                            }
                            label {
                                for: "amount",
                                "Amount (in wei):"
                            }
                            input {
                                id: "amount",
                                name: "amount",
                                r#type: "text",
                            }
                            label {
                                for: "token",
                                "Token:"
                            }
                            select {
                                id: "token",
                                name: "token",
                                option {
                                    value: "ETH",
                                    "ETH"
                                }
                                option {
                                    value: "USDC",
                                    "USDC"
                                }
                            }
                            button {
                                r#type: "submit",
                                "Withdraw"
                            }
                        }
                        "{withdraw_resp}"
                    }
                }
            }
        }
    }
}
