use std::{collections::HashMap, io::stderr};

use erc20s::ERC20S;
use revm::{
    DatabaseRef,
    primitives::{Address, Bytes, alloy_primitives::TxHash, hex},
};
use serde::{Deserialize, Serialize};
use tlock_pdk::{
    runner::PluginRunner,
    state::{set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        alloy::{
            self,
            eips::{BlockId, BlockNumberOrTag},
            network::Ethereum,
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
    alloydb::AlloyDb, provider::Provider, provider_snapshot::ProviderSnapshot,
    rpc::header_to_block_env,
};

mod alloydb;
mod chain;
mod provider;
mod provider_snapshot;
mod rpc;

#[derive(Debug, Serialize, Deserialize)]
struct State {
    fork_block: BlockId,
    chain_id: u64,
    fork_snapshot: ProviderSnapshot,

    page_id: Option<PageId>,
}

const RPC_URL: &str = "https://1rpc.io/eth";

async fn init(transport: Transport, _params: ()) -> Result<(), RpcError> {
    let mut state = State {
        fork_block: BlockId::number(0),
        chain_id: 0,
        fork_snapshot: ProviderSnapshot::default(),
        page_id: None,
    };
    handle_reset_fork(transport.clone(), &mut state).await?;
    set_state(transport.clone(), &state)?;

    //? Register the revm entities
    host::RegisterEntity
        .call_async(transport.clone(), Domain::EthProvider)
        .await?;
    let page_id = host::RegisterEntity
        .call_async(transport.clone(), Domain::Page)
        .await?;

    state.page_id = match page_id {
        EntityId::Page(id) => Some(id),
        _ => None,
    };
    set_state(transport.clone(), &state)?;

    Ok(())
}

async fn on_load(transport: Transport, page_id: PageId) -> Result<(), RpcError> {
    let state: State = try_get_state(transport.clone())?;

    let component = build_ui(&state);
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

    let mut state: State = try_get_state(transport.clone())?;

    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "reset_fork" => {
            handle_reset_fork(transport.clone(), &mut state).await?;
        }
        page::PageEvent::ButtonClicked(button_id) if button_id == "mine_fork" => {
            handle_mine(transport.clone(), &mut state)?;
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "deal_form" => {
            handle_deal(transport.clone(), &mut state, form_data)?;
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    }

    set_state(transport.clone(), &state)?;

    let component = build_ui(&state);
    host::SetPage
        .call_async(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn chain_id(transport: Transport, _: EthProviderId) -> Result<U256, RpcError> {
    let state: State = try_get_state(transport.clone())?;
    Ok(U256::from(state.chain_id))
}

async fn block_number(transport: Transport, _: EthProviderId) -> Result<u64, RpcError> {
    let fork = get_fork_provider(transport.clone())?;
    Ok(fork.block_number())
}

async fn gas_price(transport: Transport, _: EthProviderId) -> Result<u128, RpcError> {
    let fork = get_fork_provider(transport.clone())?;
    Ok(fork.gas_price())
}

async fn get_balance(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let (_, address, block_id) = params;
    let mut fork = get_fork_provider(transport.clone())?;
    let balance = fork.get_balance(address, block_id)?;
    set_snapshot(transport.clone(), fork.snapshot())?;
    Ok(balance)
}

async fn get_block(
    transport: Transport,
    params: (EthProviderId, BlockId, BlockTransactionsKind),
) -> Result<Block, RpcError> {
    let (_, block_id, tx_kind) = params;
    let fork = get_fork_provider(transport.clone())?;
    Ok(fork.get_block(block_id, tx_kind)?)
}

async fn get_code(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<Bytes, RpcError> {
    let (_, address, block_id) = params;
    let mut fork = get_fork_provider(transport.clone())?;
    let code = fork
        .get_code(address, block_id)?
        .ok_or(RpcError::Custom("Account has no code".into()))?;
    set_snapshot(transport.clone(), fork.snapshot())?;
    Ok(code)
}

async fn get_transaction_count(
    transport: Transport,
    params: (EthProviderId, Address, BlockId),
) -> Result<u64, RpcError> {
    let (_, address, block_id) = params;
    let mut fork = get_fork_provider(transport.clone())?;
    let transaction_count = fork.get_transaction_count(address, block_id)?;
    set_snapshot(transport.clone(), fork.snapshot())?;
    Ok(transaction_count)
}

async fn get_transaction_by_hash(
    transport: Transport,
    params: (EthProviderId, TxHash),
) -> Result<Transaction, RpcError> {
    let (_, tx_hash) = params;
    let fork = get_fork_provider(transport.clone())?;
    Ok(fork.get_transaction_by_hash(tx_hash)?)
}

async fn get_transaction_receipt(
    transport: Transport,
    params: (EthProviderId, TxHash),
) -> Result<TransactionReceipt, RpcError> {
    let (_, tx_hash) = params;
    let fork = get_fork_provider(transport.clone())?;
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
    let fork = get_fork_provider(transport.clone())?;
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
    let mut fork = get_fork_provider(transport.clone())?;
    let resp = fork.call(tx_request, block_id, state_override, block_override)?;
    set_snapshot(transport.clone(), fork.snapshot())?;

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
    let mut fork = get_fork_provider(transport.clone())?;
    let resp = fork.estimate_gas(tx_request, block_id, state_override, block_override)?;

    set_snapshot(transport.clone(), fork.snapshot())?;
    Ok(resp)
}

async fn send_raw_transaction(
    transport: Transport,
    params: (EthProviderId, Bytes),
) -> Result<TxHash, RpcError> {
    let (_, raw_tx) = params;
    let mut fork = get_fork_provider(transport.clone())?;
    let tx = fork.send_raw_transaction(raw_tx)?;

    set_snapshot(transport.clone(), fork.snapshot())?;

    info!("Transaction sent");
    let state: State = try_get_state(transport.clone())?;
    if let Some(page_id) = state.page_id {
        info!("Updating UI page after sending transaction");
        let component = build_ui(&state);
        host::SetPage
            .call_async(transport.clone(), (page_id, component))
            .await?;
    }

    Ok(tx)
}

async fn get_logs(
    transport: Transport,
    params: (EthProviderId, Filter),
) -> Result<Vec<Log>, RpcError> {
    let (_, filter) = params;
    let fork = get_fork_provider(transport.clone())?;
    Ok(fork.get_logs(filter)?)
}

async fn fee_history(
    transport: Transport,
    params: (EthProviderId, u64, BlockNumberOrTag, Vec<f64>),
) -> Result<alloy::rpc::types::FeeHistory, RpcError> {
    let (_, block_count, newest_block, reward_percentiles) = params;
    let fork = get_fork_provider(transport.clone())?;
    Ok(fork.fee_history(block_count, newest_block, reward_percentiles)?)
}

fn set_snapshot(transport: Transport, snapshot: ProviderSnapshot) -> Result<(), RpcError> {
    let mut state: State = try_get_state(transport.clone())?;
    state.fork_snapshot = snapshot;
    set_state(transport.clone(), &state)?;
    Ok(())
}

/// Returns a fork provider based on the saved state.
///
/// Errors if no tokio runtime is available.
fn get_fork_provider(
    transport: Transport,
) -> Result<Provider<impl DatabaseRef + std::fmt::Debug>, RpcError> {
    let state: State = try_get_state(transport.clone())?;
    let fork_block = state.fork_block;
    let fork_snapshot = state.fork_snapshot;

    let db: AlloyDb<Ethereum> = AlloyDb::new(transport.clone(), RPC_URL.to_string(), fork_block);
    let fork_provider = Provider::from_snapshot(db, fork_snapshot);
    Ok(fork_provider)
}

fn build_ui(state: &State) -> Component {
    let mut sections = vec![
        heading("REVM Provider"),
        text("A forked Ethereum provider running on REVM"),
    ];

    // Fork info section
    let current_block = state.fork_snapshot.chain.pending.env.number;
    let latest_mined = current_block.saturating_sub(U256::from(1));

    sections.extend(vec![
        heading2("Fork Information"),
        text(format!("Chain ID: {}", state.chain_id)),
        text(format!(
            "Fork Block: {:?}",
            state.fork_block.as_u64().unwrap_or(0)
        )),
        text(format!("Current Block: {}", latest_mined.to_string())),
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
    let tx_count: usize = state
        .fork_snapshot
        .transactions
        .values()
        .map(|v| v.len())
        .sum();
    sections.push(heading2("Transactions"));

    if tx_count == 0 {
        sections.push(text("No transactions"));
        return container(sections);
    }

    sections.push(text(format!("Total transactions: {}", tx_count)));

    // Show transactions by block
    let mut sorted_blocks: Vec<_> = state.fork_snapshot.transactions.iter().collect();
    sorted_blocks.sort_by_key(|(block_num, _)| *block_num);
    sections.push(unordered_list(sorted_blocks.iter().flat_map(
        |(number, txs)| {
            let block_header = (
                format!("block_{}", number),
                text(format!("Block {}: {} transaction(s)", number, txs.len())),
            );

            let tx_items = txs.iter().enumerate().map(move |(i, tx)| {
                use alloy_consensus::Transaction;
                (
                    format!("block_{}_tx_{}", number, i),
                    text(format!(
                        "  - {} -> {} | value: {} wei | sig: {}",
                        tx.inner.signer(),
                        tx.to().unwrap_or(Address::ZERO),
                        tx.value(),
                        if tx.input().len() >= 4 {
                            format!("0x{}", hex::encode(&tx.input()[..4]))
                        } else {
                            "0x".to_string()
                        }
                    )),
                )
            });

            std::iter::once(block_header).chain(tx_items)
        },
    )));

    container(sections)
}

async fn handle_reset_fork(transport: Transport, state: &mut State) -> Result<(), RpcError> {
    info!("Resetting fork to chain head");

    let db: AlloyDb<Ethereum> =
        AlloyDb::new(transport.clone(), RPC_URL.to_string(), BlockId::number(0));

    let header = db
        .get_block(BlockNumberOrTag::Latest)
        .context("Failed to get latest block")?;
    let chain_id = db.chain_id().context("Failed to get chain ID")?;

    let block_id = BlockId::number(header.number);
    let db: AlloyDb<Ethereum> = AlloyDb::new(transport.clone(), RPC_URL.to_string(), block_id);
    let parent_hash = header.parent_hash;
    let block_env = header_to_block_env(header);
    let fork = Provider::new(db, chain_id, block_env, parent_hash);
    let fork_snapshot = fork.snapshot();

    state.fork_block = block_id;
    state.fork_snapshot = fork_snapshot;
    state.chain_id = chain_id;

    Ok(())
}

fn handle_mine(transport: Transport, state: &mut State) -> Result<(), RpcError> {
    let mut fork = get_fork_provider(transport.clone())?;
    fork.mine()?;
    state.fork_snapshot = fork.snapshot();
    info!("Mined a new block on the fork");
    Ok(())
}

fn handle_deal(
    transport: Transport,
    state: &mut State,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let account: AccountId = form_data
        .get("account")
        .context("Missing account")?
        .parse()
        .context("Invalid account")?;
    let address = account
        .as_evm_address()
        .context("Account must be EVM-compatible")?;

    let amount: U256 = form_data
        .get("amount")
        .context("Missing amount")?
        .parse()
        .context("Invalid amount")?;

    let asset_symbol = form_data.get("asset").context("Missing asset")?.as_str();
    info!("Dealing {}:{} to address {}", asset_symbol, amount, address);

    let mut fork = get_fork_provider(transport.clone())?;

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

    state.fork_snapshot = fork.snapshot();

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
