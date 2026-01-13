use std::{collections::HashMap, io::stderr};

use erc20s::ERC20S;
use revm::primitives::{Address, Bytes, alloy_primitives::TxHash, hex};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    runner::PluginRunner,
    state::StateExt,
    tlock_api::{
        RpcMethod,
        alloy::{
            self,
            eips::{BlockId, BlockNumberOrTag},
            primitives::U256,
            rpc::types::{
                Block, BlockOverrides, BlockTransactionsKind, Filter, Log, Transaction,
                TransactionReceipt, TransactionRequest, state::StateOverride,
            },
        },
        caip::AccountId,
        component::{
            Component, button_input, container, dropdown, form, heading, heading2, submit_input,
            text, text_input, unordered_list,
        },
        domains::Domain,
        entities::{EntityId, EthProviderId, PageId},
        eth::{self},
        host, page, plugin,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, RpcErrorContext},
        transport::Transport,
    },
};
use tracing::{info, warn};
use tracing_subscriber::fmt;

use crate::{
    provider::Provider,
    remote_db::{get_chain_id, get_latest_block_header},
    state::get_main_key,
};

mod cache_db;
mod chain;
mod layered_db;
mod provider;
mod remote_db;
mod rpc;
mod state;

#[derive(Debug, Serialize, Deserialize)]
struct State {
    page_id: PageId,
}

const RPC_URL: &str = "https://1rpc.io/eth";
const PROVIDER_KEY: &str = "revm_fork_provider";

async fn init(transport: Transport, _params: ()) -> Result<(), RpcError> {
    handle_reset_fork(transport.clone())?;

    //? Register the revm entities
    host::RegisterEntity
        .call_async(transport.clone(), Domain::EthProvider)
        .await?;
    let page_id = host::RegisterEntity
        .call_async(transport.clone(), Domain::Page)
        .await?;

    let page_id = match page_id {
        EntityId::Page(id) => Some(id),
        _ => None,
    }
    .context("Invalid Page ID")?;

    //? Write initial state
    let state = State { page_id };
    let state_key = get_main_key(PROVIDER_KEY);
    transport.state().write_key(state_key, state)?;

    Ok(())
}

async fn on_load(transport: Transport, page_id: PageId) -> Result<(), RpcError> {
    let provider = load_provider(transport.clone())?;

    let component = build_ui(provider)?;
    host::SetPage
        .call_async(transport, (page_id, component))
        .await?;
    Ok(())
}

async fn on_update(
    transport: Transport,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated: {:?}", event);

    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "reset_fork" => {
            handle_reset_fork(transport.clone())?;
        }
        page::PageEvent::ButtonClicked(button_id) if button_id == "mine_fork" => {
            handle_mine(transport.clone())?;
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "deal_form" => {
            handle_deal(transport.clone(), form_data)?;
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    }

    let provider = load_provider(transport.clone())?;
    let component = build_ui(provider)?;
    host::SetPage
        .call_async(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn chain_id(transport: Transport, _: EthProviderId) -> Result<U256, RpcError> {
    let fork = load_provider(transport.clone())?;
    Ok(U256::from(fork.state.chain_id))
}

async fn block_number(transport: Transport, _: EthProviderId) -> Result<u64, RpcError> {
    let fork = load_provider(transport.clone())?;
    Ok(fork.block_number()?)
}

async fn gas_price(transport: Transport, _: EthProviderId) -> Result<u128, RpcError> {
    let fork = load_provider(transport.clone())?;
    Ok(fork.gas_price()?)
}

async fn get_balance(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let (_, address, block_id) = params;
    let fork = load_provider(transport.clone())?;
    let balance = fork.get_balance(address, block_id)?;
    Ok(balance)
}

async fn get_block(
    transport: Transport,
    params: (EthProviderId, BlockId, BlockTransactionsKind),
) -> Result<Block, RpcError> {
    let (_, block_id, tx_kind) = params;
    let fork = load_provider(transport.clone())?;
    Ok(fork.get_block(block_id, tx_kind)?)
}

async fn get_code(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<Bytes, RpcError> {
    let (_, address, block_id) = params;
    let fork = load_provider(transport.clone())?;
    let code = fork.get_code(address, block_id)?.unwrap_or_default();
    Ok(code)
}

async fn get_transaction_count(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<u64, RpcError> {
    let (_, address, block_id) = params;
    let fork = load_provider(transport.clone())?;
    let transaction_count = fork.get_transaction_count(address, block_id)?;
    Ok(transaction_count)
}

async fn get_transaction_by_hash(
    transport: Transport,
    params: (EthProviderId, TxHash),
) -> Result<Transaction, RpcError> {
    let (_, tx_hash) = params;
    let fork = load_provider(transport.clone())?;
    Ok(fork.get_transaction_by_hash(tx_hash)?)
}

async fn get_transaction_receipt(
    transport: Transport,
    params: (EthProviderId, TxHash),
) -> Result<TransactionReceipt, RpcError> {
    let (_, tx_hash) = params;
    let fork = load_provider(transport.clone())?;
    let transaction_receipt = fork
        .get_transaction_receipt(tx_hash)
        .ok_or(RpcError::Custom("Transaction receipt not found".into()))?;
    Ok(transaction_receipt)
}

async fn get_block_receipts(
    transport: Transport,
    params: (EthProviderId, BlockId),
) -> Result<Vec<TransactionReceipt>, RpcError> {
    let (_, block_id) = params;
    let fork = load_provider(transport.clone())?;
    Ok(fork.get_block_receipts(block_id)?)
}

async fn call(
    transport: Transport,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<Bytes, RpcError> {
    let (_, tx_request, block_id, state_override, block_override) = params;
    let fork = load_provider(transport.clone())?;
    let resp = fork.call(tx_request, block_id, state_override, block_override)?;
    Ok(resp)
}

async fn estimate_gas(
    transport: Transport,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<u64, RpcError> {
    let (_, tx_request, block_id, state_override, block_override) = params;
    let fork = load_provider(transport.clone())?;
    let resp = fork.estimate_gas(tx_request, block_id, state_override, block_override)?;
    Ok(resp)
}

async fn send_raw_transaction(
    transport: Transport,
    params: (EthProviderId, Bytes),
) -> Result<TxHash, RpcError> {
    let (_, raw_tx) = params;
    let fork = load_provider(transport.clone())?;
    let tx = fork.send_raw_transaction(raw_tx)?;

    info!("Transaction sent");
    let state_key = get_main_key(PROVIDER_KEY);
    let state: State = transport.state().read_key(state_key)?;

    let provider = load_provider(transport.clone())?;
    let component = build_ui(provider)?;
    host::SetPage
        .call_async(transport.clone(), (state.page_id, component))
        .await?;

    Ok(tx)
}

async fn get_logs(
    transport: Transport,
    params: (EthProviderId, Filter),
) -> Result<Vec<Log>, RpcError> {
    let (_, filter) = params;
    let fork = load_provider(transport.clone())?;
    Ok(fork.get_logs(filter)?)
}

async fn fee_history(
    transport: Transport,
    params: (EthProviderId, u64, BlockNumberOrTag, Vec<f64>),
) -> Result<alloy::rpc::types::FeeHistory, RpcError> {
    let (_, block_count, newest_block, reward_percentiles) = params;
    let fork = load_provider(transport.clone())?;
    Ok(fork.fee_history(block_count, newest_block, reward_percentiles)?)
}

/// Returns a fork provider based on the saved state.
///
/// Errors if no tokio runtime is available.
fn load_provider(transport: Transport) -> Result<Provider, RpcError> {
    Ok(Provider::load(
        transport,
        PROVIDER_KEY.to_string(),
        RPC_URL.to_string(),
    )?)
}

fn build_ui(provider: Provider) -> Result<Component, RpcError> {
    let mut sections = vec![
        heading("REVM Provider"),
        text("A forked Ethereum provider running on REVM"),
    ];

    // Fork info section
    let chain_id = provider.state.chain_id;
    let fork_block = provider.state.fork_block;
    let latest_block = provider.block_number()? - 1;
    sections.extend(vec![
        heading2("Fork Information"),
        text(format!("Chain ID: {}", chain_id)),
        text(format!("Fork Block: {:?}", fork_block)),
        text(format!("Current Block: {}", latest_block)),
        button_input("mine_fork", "Mine"),
        button_input("reset_fork", "Reset Fork to Chain Head"),
    ]);

    // Cheatcodes section
    let mut asset_symbols = vec!["ETH".to_string()];
    asset_symbols.extend(ERC20S.iter().map(|e| e.symbol.to_string()));
    sections.extend(vec![
        heading2("Cheatcodes"),
        heading2("Deal"),
        text("Sets the balance of an account"),
        form(
            "deal_form",
            vec![
                text_input("account", "Account", "eip155:1:0xabc123..."),
                text_input("amount", "Amount (wei)", "10000"),
                dropdown("asset", "Asset", asset_symbols, Some("ETH".to_string())),
                submit_input("Execute Deal"),
            ],
        ),
    ]);

    // Transactions section
    let tx_count: usize = provider.state.transactions.values().map(|v| v.len()).sum();
    sections.push(heading2("Blocks"));

    sections.push(text(format!("Total transactions: {}", tx_count)));

    // Show transactions by block
    let mut sorted_blocks: Vec<_> = provider.state.transactions.iter().collect();
    sorted_blocks.sort_by_key(|(block_num, _)| *block_num);
    let receipts = provider.state.receipts.clone();
    sections.push(unordered_list(sorted_blocks.iter().flat_map(
        |(number, txs)| {
            let block_header = (
                format!("block_{}", number),
                text(format!("Block {}: {} transaction(s)", number, txs.len())),
            );

            let receipts = receipts.clone();
            let tx_items = txs.iter().enumerate().map(move |(i, tx)| {
                use alloy_consensus::Transaction;

                let tx_hash = tx.inner.hash();
                let receipt = receipts.get(tx_hash);
                let status_icon = match receipt {
                    Some(r) if r.status() => "✓",
                    Some(_) => "✗",
                    None => "?",
                };

                let sig_display = if tx.input().len() >= 4 {
                    format!("0x{}", hex::encode(&tx.input()[..4]))
                } else {
                    "0x".to_string()
                };

                let mut display = format!(
                    "  - {} {} -> {} | value: {} wei | sig: {}",
                    status_icon,
                    tx.inner.signer(),
                    tx.to().unwrap_or(Address::ZERO),
                    tx.value(),
                    sig_display
                );

                if let Some(r) = receipt {
                    if !r.status() {
                        display.push_str(" | REVERTED");
                    }
                }

                (
                    format!("block_{}_tx_{}", number, i),
                    text(display),
                )
            });

            std::iter::once(block_header).chain(tx_items)
        },
    )));

    Ok(container(sections))
}

fn handle_reset_fork(transport: Transport) -> Result<(), RpcError> {
    let chain_id =
        get_chain_id(transport.clone(), RPC_URL.to_string()).context("Error getting chain_id")?;
    let header = get_latest_block_header(transport.clone(), RPC_URL.to_string())
        .context("Error getting block_number")?;

    //? Create a new provide to initialize the fork state
    let _ = Provider::new(
        transport,
        PROVIDER_KEY.to_string(),
        RPC_URL.to_string(),
        header,
        chain_id,
        12,
    )?;

    Ok(())
}

fn handle_mine(transport: Transport) -> Result<(), RpcError> {
    let fork = load_provider(transport.clone())?;
    fork.mine()?;
    Ok(())
}

fn handle_deal(transport: Transport, form_data: HashMap<String, String>) -> Result<(), RpcError> {
    let account: AccountId = form_data
        .get("account")
        .context("Missing account")?
        .parse()
        .context("Invalid account")?;
    let address = account
        .as_evm_address()
        .context("Account must be an EVM address")?;

    let amount: U256 = form_data
        .get("amount")
        .context("Missing amount")?
        .parse()
        .context("Invalid amount")?;

    let asset_symbol = form_data.get("asset").context("Missing asset")?.as_str();
    info!("Dealing {}:{} to address {}", asset_symbol, amount, address);

    let fork = load_provider(transport.clone())?;
    match asset_symbol {
        "ETH" => {
            fork.deal(address, amount)?;
        }
        other => {
            let token_address = ERC20S
                .iter()
                .find(|e| e.symbol == other)
                .map(|e| e.address)
                .context("Unknown ERC20 asset")?;
            fork.deal_erc20(address, token_address, amount)?;
        }
    }

    Ok(())
}

// TODO: See if there's a way to use anvil instead of revm directly + my own
// semi-hacky impl.
fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();

    PluginRunner::new()
        .with_method(plugin::Init, init)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .with_method(eth::ChainId, chain_id)
        .with_method(eth::BlockNumber, block_number)
        .with_method(eth::GasPrice, gas_price)
        .with_method(eth::GetBalance, get_balance)
        .with_method(eth::GetBlock, get_block)
        .with_method(eth::GetCode, get_code)
        .with_method(eth::GetTransactionCount, get_transaction_count)
        .with_method(eth::GetTransactionByHash, get_transaction_by_hash)
        .with_method(eth::GetTransactionReceipt, get_transaction_receipt)
        .with_method(eth::GetBlockReceipts, get_block_receipts)
        .with_method(eth::Call, call)
        .with_method(eth::EstimateGas, estimate_gas)
        .with_method(eth::SendRawTransaction, send_raw_transaction)
        .with_method(eth::GetLogs, get_logs)
        .with_method(eth::FeeHistory, fee_history)
        .run();
}
