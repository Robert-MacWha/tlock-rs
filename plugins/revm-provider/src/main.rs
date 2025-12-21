use std::{collections::HashMap, io::stderr, sync::Arc};

use revm::{
    DatabaseRef,
    primitives::{Address, Bytes, alloy_primitives::TxHash},
};
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    server::PluginServer,
    state::{set_state, try_get_state},
    tlock_api::{
        RpcMethod,
        alloy::{
            eips::BlockId,
            primitives::U256,
            providers::{Provider as AlloyProvider, ProviderBuilder},
            rpc::types::{
                Block, BlockOverrides, BlockTransactionsKind, Filter, Log, Transaction,
                TransactionReceipt, TransactionRequest, state::StateOverride,
            },
        },
        caip::ChainId,
        component::{
            Component, button_input, container, form, heading, heading2, submit_input, text,
            text_input, unordered_list,
        },
        domains::Domain,
        entities::{EthProviderId, PageId},
        eth::{self},
        host, page, plugin,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, to_rpc_err},
        transport::JsonRpcTransport,
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
    alloy_provider_id: EthProviderId,
    fork_block: BlockId,

    fork_snapshot: ProviderSnapshot,
}

const CHAIN_ID: u64 = 11155111u64;

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    // TODO: Consider embedding the alloy instance in this plugin rather than using
    // an eth provider. Saves the cost of inter-plugin calls.
    let provider_id = host::RequestEthProvider
        .call(transport.clone(), ChainId::new_evm(CHAIN_ID))
        .await?;

    //? Setup the forked provider
    let alloy =
        ProviderBuilder::new().connect_client(AlloyBridge::new(transport.clone(), provider_id));
    let block = alloy
        .get_block(BlockId::latest())
        .await
        .map_err(to_rpc_err)?
        .ok_or(RpcError::Custom("Failed to get latest block".into()))?;

    let fork_block_id = BlockId::number(block.number());
    let db = AlloyDb::new(alloy, fork_block_id);

    let parent_hash = block.header.parent_hash;
    let block_env = header_to_block_env(block.header);

    let fork = Provider::new(db, block_env, parent_hash);
    let fork_snapshot = fork.snapshot();

    //? Save setup snapshot
    let state = State {
        alloy_provider_id: provider_id,
        fork_block: fork_block_id,
        fork_snapshot,
    };
    set_state(transport.clone(), &state).await?;

    //? Register the revm entities
    host::RegisterEntity
        .call(transport.clone(), Domain::EthProvider)
        .await?;
    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    Ok(())
}

async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    let state: State = try_get_state(transport.clone()).await.map_err(to_rpc_err)?;

    let component = build_ui(&state);
    host::SetPage.call(transport, (page_id, component)).await?;
    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated: {:?}", event);

    let mut state: State = try_get_state(transport.clone()).await.map_err(to_rpc_err)?;

    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "reset_fork" => {
            handle_reset_fork(&transport, &mut state).await?;
        }
        page::PageEvent::ButtonClicked(button_id) if button_id == "mine_fork" => {
            handle_mine(&transport, &mut state).await?;
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "deal_form" => {
            handle_deal(&transport, &mut state, form_data).await?;
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "deal_erc20_form" => {
            handle_deal_erc20(&transport, &mut state, form_data).await?;
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    }

    set_state(transport.clone(), &state).await?;

    let component = build_ui(&state);
    host::SetPage
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn chain_id(_: Arc<JsonRpcTransport>, _: EthProviderId) -> Result<U256, RpcError> {
    Ok(U256::from(CHAIN_ID))
}

async fn block_number(transport: Arc<JsonRpcTransport>, _: EthProviderId) -> Result<u64, RpcError> {
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.block_number())
}

async fn gas_price(transport: Arc<JsonRpcTransport>, _: EthProviderId) -> Result<u128, RpcError> {
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.gas_price())
}

async fn get_balance(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<U256, RpcError> {
    let (_, address, block_id) = params;
    let mut fork = get_fork_provider(transport.clone()).await?;
    let balance = fork.get_balance(address, block_id)?;
    set_snapshot(transport.clone(), fork.snapshot()).await?;
    Ok(balance)
}

async fn get_block(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, BlockId, BlockTransactionsKind),
) -> Result<Block, RpcError> {
    let (_, block_id, tx_kind) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_block(block_id, tx_kind)?)
}

async fn get_code(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<Bytes, RpcError> {
    let (_, address, block_id) = params;
    let mut fork = get_fork_provider(transport.clone()).await?;
    let code = fork
        .get_code(address, block_id)?
        .ok_or(RpcError::Custom("Account has no code".into()))?;
    set_snapshot(transport.clone(), fork.snapshot()).await?;
    Ok(code)
}

async fn get_transaction_count(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Address, BlockId),
) -> Result<u64, RpcError> {
    let (_, address, block_id) = params;
    let mut fork = get_fork_provider(transport.clone()).await?;
    let transaction_count = fork.get_transaction_count(address, block_id)?;
    set_snapshot(transport.clone(), fork.snapshot()).await?;
    Ok(transaction_count)
}

async fn get_transaction_by_hash(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, TxHash),
) -> Result<Transaction, RpcError> {
    let (_, tx_hash) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_transaction_by_hash(tx_hash)?)
}

async fn get_transaction_receipt(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, TxHash),
) -> Result<TransactionReceipt, RpcError> {
    let (_, tx_hash) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    let transaction_receipt = fork
        .get_transaction_receipt(tx_hash)
        .ok_or(RpcError::Custom("Transaction receipt not found".into()))?;
    Ok(transaction_receipt)
}

async fn get_block_receipts(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, BlockId),
) -> Result<Vec<TransactionReceipt>, RpcError> {
    let (_, block_id) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_block_receipts(block_id)?)
}

async fn call(
    transport: Arc<JsonRpcTransport>,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<Bytes, RpcError> {
    let (_, tx_request, block_id, state_override, block_override) = params;
    let fork = get_fork_provider(transport.clone()).await?;

    set_snapshot(transport.clone(), fork.snapshot()).await?;
    Ok(fork.call(tx_request, block_id, state_override, block_override)?)
}

async fn estimate_gas(
    transport: Arc<JsonRpcTransport>,
    params: (
        EthProviderId,
        TransactionRequest,
        BlockId,
        Option<StateOverride>,
        Option<BlockOverrides>,
    ),
) -> Result<u64, RpcError> {
    let (_, tx_request, block_id, state_override, block_override) = params;
    let fork = get_fork_provider(transport.clone()).await?;

    set_snapshot(transport.clone(), fork.snapshot()).await?;
    Ok(fork.estimate_gas(tx_request, block_id, state_override, block_override)?)
}

async fn send_raw_transaction(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Bytes),
) -> Result<TxHash, RpcError> {
    let (_, raw_tx) = params;
    let mut fork = get_fork_provider(transport.clone()).await?;
    let tx = fork.send_raw_transaction(raw_tx)?;

    set_snapshot(transport.clone(), fork.snapshot()).await?;
    Ok(tx)
}

async fn get_logs(
    transport: Arc<JsonRpcTransport>,
    params: (EthProviderId, Filter),
) -> Result<Vec<Log>, RpcError> {
    let (_, filter) = params;
    let fork = get_fork_provider(transport.clone()).await?;
    Ok(fork.get_logs(filter)?)
}

async fn set_snapshot(
    transport: Arc<JsonRpcTransport>,
    snapshot: ProviderSnapshot,
) -> Result<(), RpcError> {
    let mut state: State = try_get_state(transport.clone()).await.map_err(to_rpc_err)?;
    state.fork_snapshot = snapshot;
    set_state(transport.clone(), &state).await?;
    Ok(())
}

/// Returns a fork provider based on the saved state.
///
/// Errors if no tokio runtime is available.
async fn get_fork_provider(
    transport: Arc<JsonRpcTransport>,
) -> Result<Provider<impl DatabaseRef + std::fmt::Debug>, RpcError> {
    let state: State = try_get_state(transport.clone()).await.map_err(to_rpc_err)?;
    let alloy_provider_id = state.alloy_provider_id;
    let fork_block = state.fork_block;
    let fork_snapshot = state.fork_snapshot;

    let alloy = ProviderBuilder::new()
        .connect_client(AlloyBridge::new(transport.clone(), alloy_provider_id));

    let db = AlloyDb::new(alloy, fork_block);
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
        text(format!(
            "Fork Block: {:?}",
            state.fork_block.as_u64().unwrap_or(0)
        )),
        text(format!("Current Block: {}", latest_mined.to_string())),
        button_input("mine_fork", "Mine"),
        button_input("reset_fork", "Reset Fork to Chain Head"),
    ]);

    // Cheatcodes section
    sections.extend(vec![
        heading2("Cheatcodes"),
        heading2("Deal"),
        text("Set native ETH balance for an address"),
        form(
            "deal_form",
            vec![
                text_input("address", "Address (hex)"),
                text_input("amount", "Amount (wei)"),
                submit_input("Execute Deal"),
            ],
        ),
        heading2("Deal ERC20"),
        text("Set ERC20 token balance for an address"),
        form(
            "deal_erc20_form",
            vec![
                text_input("address", "Holder Address (hex)"),
                text_input("erc20", "ERC20 Contract Address (hex)"),
                text_input("amount", "Amount (wei)"),
                submit_input("Execute Deal ERC20"),
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
    sections.push(unordered_list(sorted_blocks.iter().map(|(number, txs)| {
        (
            format!("block_{}", number),
            text(format!("Block {}: {} transaction(s)", number, txs.len())),
        )
    })));

    container(sections)
}

async fn handle_reset_fork(
    transport: &Arc<JsonRpcTransport>,
    state: &mut State,
) -> Result<(), RpcError> {
    info!("Resetting fork to chain head");

    let alloy = ProviderBuilder::new()
        .connect_client(AlloyBridge::new(transport.clone(), state.alloy_provider_id));

    let block = alloy
        .get_block(BlockId::latest())
        .await
        .map_err(to_rpc_err)?
        .ok_or(RpcError::Custom("Failed to get latest block".into()))?;
    let fork_block_id = BlockId::number(block.number());

    let db = AlloyDb::new(alloy, fork_block_id);
    let parent_hash = block.header.parent_hash;
    let block_env = header_to_block_env(block.header);
    let fork = Provider::new(db, block_env, parent_hash);
    let fork_snapshot = fork.snapshot();

    state.fork_block = fork_block_id;
    state.fork_snapshot = fork_snapshot;

    Ok(())
}

async fn handle_mine(transport: &Arc<JsonRpcTransport>, state: &mut State) -> Result<(), RpcError> {
    let mut fork = get_fork_provider(transport.clone()).await?;
    fork.mine()?;
    state.fork_snapshot = fork.snapshot();
    info!("Mined a new block on the fork");
    Ok(())
}

async fn handle_deal(
    transport: &Arc<JsonRpcTransport>,
    state: &mut State,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let address: Address = form_data
        .get("address")
        .ok_or(RpcError::Custom("Missing address".into()))?
        .parse()
        .map_err(|e| RpcError::Custom(format!("Invalid address: {}", e)))?;

    let amount: U256 = form_data
        .get("amount")
        .ok_or(RpcError::Custom("Missing amount".into()))?
        .parse()
        .map_err(|_| RpcError::Custom("Invalid amount".into()))?;

    info!(
        "Executing deal for address {} with amount {}",
        address, amount
    );

    let mut fork = get_fork_provider(transport.clone()).await?;
    fork.deal(address, amount)?;
    state.fork_snapshot = fork.snapshot();

    Ok(())
}

async fn handle_deal_erc20(
    transport: &Arc<JsonRpcTransport>,
    state: &mut State,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let address: Address = form_data
        .get("address")
        .ok_or(RpcError::Custom("Missing address".into()))?
        .parse()
        .map_err(|e| RpcError::Custom(format!("Invalid address: {}", e)))?;

    let erc20: Address = form_data
        .get("erc20")
        .ok_or(RpcError::Custom("Missing erc20 contract address".into()))?
        .parse()
        .map_err(|e| RpcError::Custom(format!("Invalid erc20 address: {}", e)))?;

    let amount: U256 = form_data
        .get("amount")
        .ok_or(RpcError::Custom("Missing amount".into()))?
        .parse()
        .map_err(|_| RpcError::Custom("Invalid amount".into()))?;

    info!(
        "Executing deal_erc20 for address {} with erc20 {} and amount {}",
        address, erc20, amount
    );

    let mut fork = get_fork_provider(transport.clone()).await?;
    fork.deal_erc20(address, erc20, amount)?;
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

    info!("Starting plugin...");
    PluginServer::new_with_transport()
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
        .run();
}
