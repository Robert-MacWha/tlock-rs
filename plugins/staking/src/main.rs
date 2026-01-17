//! Staking
//!
//! This plugin allows users to stake their ETH in a custodial vault. It
//! demonstrates how tlock can keep track of custodially held assets and
//! incorperates them into the broader vaults framework.

use std::{collections::HashMap, io::stderr};

use alloy::{
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use erc20s::CHAIN_ID;
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    runner::PluginRunner,
    state::StateExt,
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId, ChainId},
        component::{asset, container, form, heading, heading2, submit_input, text, text_input},
        domains::Domain,
        entities::{EthProviderId, PageId, VaultId},
        global, host,
        page::{self},
        plugin, vault,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, RpcErrorContext, ToRpcResult},
        transport::Transport,
    },
};
use tracing::{info, warn};
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize, Debug)]
struct PluginState {
    provider_id: EthProviderId,
    staked: U256,
    private_key: FixedBytes<32>,
    address: Address,
}

async fn init(transport: Transport, _params: ()) -> Result<(), RpcError> {
    info!("Initializing Staking Plugin");

    let provider_id =
        host::RequestEthProvider.call(transport.clone(), ChainId::new_evm(CHAIN_ID))?;
    // TODO: Enable me. Disabled for the demo to simplify things
    // host::RegisterEntity.call(transport.clone(), Domain::Vault)?;
    host::RegisterEntity.call(transport.clone(), Domain::Page)?;

    let signer = PrivateKeySigner::random();
    let address = signer.address();
    let state = PluginState {
        provider_id,
        staked: U256::ZERO,
        private_key: signer.to_bytes(),
        address,
    };

    transport.state().lock_or(|| state)?;

    Ok(())
}

async fn ping(transport: Transport, _params: ()) -> Result<String, RpcError> {
    global::Ping.call(transport, ())?;
    Ok("pong".to_string())
}

// ---------- Page Handlers ----------

async fn get_deposit_address(
    transport: Transport,
    params: (VaultId, AssetId),
) -> Result<AccountId, RpcError> {
    let (_vault_id, asset_id) = params;
    let state: PluginState = transport.state().read()?;
    if asset_id != AssetId::eth(CHAIN_ID) {
        return Err(RpcError::custom("Unsupported asset"));
    }
    let account_id = AccountId::new_evm(CHAIN_ID, state.address);
    Ok(account_id)
}

async fn get_assets(
    transport: Transport,
    _vault_id: VaultId,
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let state: PluginState = transport.state().read()?;
    Ok(vec![(AssetId::eth(CHAIN_ID), state.staked)])
}

async fn on_load(transport: Transport, page_id: PageId) -> Result<(), RpcError> {
    info!("Page loaded: {}", page_id);

    let state: PluginState = transport.state().read()?;
    let component = build_ui(&state);
    host::SetPage.call(transport.clone(), (page_id, component))?;

    Ok(())
}

async fn on_update(
    transport: Transport,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated: {:?}", event);

    match event {
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "stake_form" => {
            handle_stake(&transport, form_data)?;
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "unstake_form" => {
            handle_unstake(&transport, form_data).await?;
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    }

    let state = transport.state().read()?;
    let component = build_ui(&state);
    host::SetPage.call(transport.clone(), (page_id, component))?;

    Ok(())
}

fn handle_stake(transport: &Transport, form_data: HashMap<String, String>) -> Result<(), RpcError> {
    let amount = form_data.get("amount").context("Missing amount")?;
    let amount: f64 = amount.parse().context("Invalid amount")?;
    let amount_uint = U256::from(amount * 1e18);

    let state: PluginState = transport.state().read()?;

    let vault_id = host::RequestVault
        .call(transport.clone(), ())
        .context("Failed to request vault")?;

    let account_id = AccountId::new_evm(CHAIN_ID, state.address);
    let asset_id = AssetId::eth(CHAIN_ID);

    vault::Withdraw
        .call(
            transport.clone(),
            (vault_id, account_id, asset_id, amount_uint),
        )
        .context("Failed to withdraw from vault")?;

    host::Notify.call(
        transport.clone(),
        (host::NotifyLevel::Info, format!("Staked {:.4} ETH", amount)),
    )?;

    {
        let mut state = transport.state().try_lock::<PluginState>()?;
        state.staked += amount_uint;
    }

    Ok(())
}

async fn handle_unstake(
    transport: &Transport,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let state: PluginState = transport.state().read()?;

    let amount = form_data.get("amount").context("Missing amount")?;
    let amount: f64 = amount.parse().context("Invalid amount")?;
    let amount_uint = U256::from(amount * 1e18);
    if amount_uint > state.staked {
        return Err(RpcError::custom("Insufficient staked balance"));
    }

    let vault_id = host::RequestVault
        .call(transport.clone(), ())
        .context("Failed to request vault")?;

    let asset_id = AssetId::eth(CHAIN_ID);
    let deposit_address = vault::GetDepositAddress
        .call(transport.clone(), (vault_id, asset_id))
        .context("Failed to get deposit address")?;

    if deposit_address.chain_id() != &ChainId::new_evm(CHAIN_ID) {
        return Err(RpcError::custom("Deposit address is not on expected chain"));
    }
    let deposit_address = deposit_address
        .as_evm_address()
        .context("Cannot withdraw to non-evm address")?;

    let signer = PrivateKeySigner::from_bytes(&state.private_key).context("Invalid private key")?;

    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_client(AlloyBridge::new(transport.clone(), state.provider_id));

    let tx = TransactionRequest::default()
        .to(deposit_address)
        .value(amount_uint);

    provider
        .send_transaction(tx)
        .await
        .rpc_err()?
        .watch()
        .await
        .rpc_err()?;

    let bal = provider.get_balance(state.address).await.rpc_err()?;
    info!("New balance after unstake: {}", bal);

    host::Notify.call(
        transport.clone(),
        (
            host::NotifyLevel::Info,
            format!("Unstaked {:.4} ETH", amount),
        ),
    )?;

    {
        let mut state = transport.state().try_lock::<PluginState>()?;
        state.staked = bal;
    }

    Ok(())
}

fn build_ui(state: &PluginState) -> tlock_pdk::tlock_api::component::Component {
    let mut sections = vec![
        heading("Custodial Staker"),
        text("Stake your ETH in a custodial vault managed by this plugin."),
    ];

    sections.push(heading2("Staked Balance"));
    sections.push(text("Staked"));
    sections.push(asset(AssetId::eth(CHAIN_ID), Some(state.staked)));

    sections.push(heading2("Stake ETH"));
    sections.push(form(
        "stake_form",
        vec![
            text_input("amount", "Amount to stake", "1.0"),
            submit_input("Stake"),
        ],
    ));

    sections.push(heading2("Unstake ETH"));
    sections.push(form(
        "unstake_form",
        vec![
            text_input("amount", "Amount to unstake", "1.0"),
            submit_input("Unstake"),
        ],
    ));

    container(sections)
}

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting Staking Plugin...");

    PluginRunner::new()
        .with_method(plugin::Init, init)
        .with_method(global::Ping, ping)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .with_method(vault::GetDepositAddress, get_deposit_address)
        .with_method(vault::GetAssets, get_assets)
        .run();
}
