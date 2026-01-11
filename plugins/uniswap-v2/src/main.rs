//! Uniswap V2 Plugin
//!
//! This plugin enables users to swap ERC20 tokens on Sepolia testnet using
//! Uniswap V2. It fetches on-chain reserves to calculate expected output
//! amounts and executes swaps via a coordinator account.

use std::{collections::HashMap, io::stderr};

use alloy::{
    primitives::{Address, U256, address},
    providers::ProviderBuilder,
    sol,
    sol_types::SolCall,
};
use erc20s::{CHAIN_ID, ERC20S};
use serde::{Deserialize, Serialize};
use tlock_alloy::AlloyBridge;
use tlock_pdk::{
    runner::PluginRunner,
    state::StateExt,
    tlock_api::{
        RpcMethod,
        caip::{AssetId, AssetType, ChainId},
        component::{
            button_input, container, dropdown, form, heading, submit_input, text, text_input,
        },
        coordinator,
        domains::Domain,
        entities::{CoordinatorId, EthProviderId, PageId},
        global, host, page, plugin,
    },
    wasmi_plugin_pdk::{
        rpc_message::{RpcError, RpcErrorContext, ToRpcResult},
        transport::Transport,
    },
};
use tracing::{error, info, warn};
use tracing_subscriber::fmt;

// ---------- Constants ----------
const UNISWAP_V2_ROUTER: Address = address!("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D");
const UNISWAP_V2_FACTORY: Address = address!("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f");

// ---------- Plugin State ----------

#[derive(Serialize, Deserialize, Debug)]
struct PluginState {
    coordinator_id: CoordinatorId,
    provider_id: EthProviderId,
    page_id: PageId,
    selected_from_token: Option<usize>,
    selected_to_token: Option<usize>,
    input_amount: f64,
    expected_output: f64,
    input_decimals: u8,
    output_decimals: u8,
    last_message: Option<String>,
}

// ---------- Alloy Contract Interfaces ----------

sol! {
    #[sol(rpc)]
    contract IUniswapV2Router02 {
        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            address[] calldata path,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);
    }

    #[sol(rpc)]
    contract IUniswapV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }

    #[sol(rpc)]
    contract IUniswapV2Pair {
        function getReserves() external view returns (
            uint112 reserve0,
            uint112 reserve1,
            uint32 blockTimestampLast
        );
    }

    #[sol(rpc)]
    contract ERC20 {
        function approve(address spender, uint256 amount) external returns (bool);
        function decimals() external view returns (uint8);
    }
}

// ---------- Plugin Handlers ----------

async fn init(transport: Transport, _params: ()) -> Result<(), RpcError> {
    info!("Initializing Uniswap V2 Plugin");

    let provider_id = host::RequestEthProvider
        .call_async(transport.clone(), ChainId::new_evm(CHAIN_ID))
        .await?;
    let coordinator_id = host::RequestCoordinator
        .call_async(transport.clone(), ())
        .await?;

    let page_id = host::RegisterEntity
        .call_async(transport.clone(), Domain::Page)
        .await?;

    let page_id = match page_id {
        tlock_pdk::tlock_api::entities::EntityId::Page(id) => Some(id),
        _ => None,
    }
    .context("Invalid Page ID")?;

    let state = PluginState {
        coordinator_id,
        provider_id,
        page_id: page_id,
        selected_from_token: None,
        selected_to_token: None,
        input_amount: 0.0,
        expected_output: 0.0,
        input_decimals: 0,
        output_decimals: 0,
        last_message: None,
    };

    transport.state().lock_or(|| state)?;

    Ok(())
}

async fn ping(transport: Transport, _params: ()) -> Result<String, RpcError> {
    global::Ping.call_async(transport, ()).await?;
    Ok("pong".to_string())
}

// ---------- Page Handlers ----------

async fn on_load(transport: Transport, page_id: PageId) -> Result<(), RpcError> {
    info!("Page loaded: {}", page_id);

    let state: PluginState = transport.state().read()?;
    let component = build_ui(&state);
    host::SetPage
        .call_async(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Transport,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated: {:?}", event);

    let mut state = transport.state().try_lock::<PluginState>()?;
    match event {
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "swap_form" => {
            handle_swap_form_update(&transport, &mut state, form_data).await?;
        }
        page::PageEvent::ButtonClicked(button_id) if button_id == "execute_swap" => {
            handle_execute_swap(&transport, &mut state).await?;
        }
        page::PageEvent::ButtonClicked(button_id) if button_id == "refresh_quote" => {
            handle_refresh_quote(&transport, &mut state).await?;
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
            return Ok(());
        }
    }

    let component = build_ui(&state);
    host::SetPage
        .call_async(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

// ---------- Event Handler Functions ----------

async fn handle_swap_form_update(
    transport: &Transport,
    state: &mut PluginState,
    form_data: HashMap<String, String>,
) -> Result<(), RpcError> {
    let Some(from_token) = form_data.get("from_token") else {
        error!("From token field missing in form data");
        return Ok(());
    };

    let Some(to_token) = form_data.get("to_token") else {
        error!("To token field missing in form data");
        return Ok(());
    };

    let Some(amount) = form_data.get("amount") else {
        error!("Amount field missing in form data");
        return Ok(());
    };

    if amount.is_empty() {
        state.input_amount = 0.0;
        state.expected_output = 0.0;
        warn!("Amount is empty, cannot calculate quote");
        return Ok(());
    }

    let Ok(amount) = amount.parse::<f64>() else {
        state.last_message = Some("Invalid amount - must be a valid number".into());
        return Ok(());
    };

    state.selected_from_token = from_token
        .split(':')
        .next()
        .and_then(|idx_str| idx_str.trim().parse::<usize>().ok());
    state.selected_to_token = to_token
        .split(':')
        .next()
        .and_then(|idx_str| idx_str.trim().parse::<usize>().ok());

    state.input_amount = amount;

    if state.selected_from_token.is_some() && state.selected_to_token.is_some() {
        state.input_amount = amount;
        calculate_quote(transport, state).await?;
    } else {
        state.last_message = Some("Select both tokens to calculate quote".into());
    }

    Ok(())
}

async fn handle_refresh_quote(
    transport: &Transport,
    state: &mut PluginState,
) -> Result<(), RpcError> {
    if state.selected_from_token.is_some()
        && state.selected_to_token.is_some()
        && state.input_amount != 0.0
    {
        calculate_quote(transport, state).await?;
        state.last_message = Some("Quote refreshed".into());
    } else {
        state.last_message = Some("Fill all fields first".into());
    }
    Ok(())
}

async fn calculate_quote(transport: &Transport, state: &mut PluginState) -> Result<(), RpcError> {
    let Some(from_idx) = state.selected_from_token else {
        error!("From token not selected");
        return Ok(());
    };
    let Some(to_idx) = state.selected_to_token else {
        error!("To token not selected");
        return Ok(());
    };

    if from_idx == to_idx {
        state.last_message = Some("Cannot swap same token".into());
        return Ok(());
    }

    let from_token = &ERC20S[from_idx];
    let to_token = &ERC20S[to_idx];

    // Parse input amount
    let amount_in = state.input_amount;

    if amount_in == 0.0 {
        error!("Input amount is zero");
        state.expected_output = 0.0;
        return Ok(());
    }

    // Get provider
    let provider = ProviderBuilder::new()
        .connect_client(AlloyBridge::new(transport.clone(), state.provider_id));

    // Get token decimals
    let from_token_contract = ERC20::new(from_token.address, &provider);
    let to_token_contract = ERC20::new(to_token.address, &provider);

    let from_decimals = from_token_contract.decimals().call().await.rpc_err()?;
    let to_decimals = to_token_contract.decimals().call().await.rpc_err()?;

    state.input_decimals = from_decimals;
    state.output_decimals = to_decimals;

    // Get pair address from factory
    let factory = IUniswapV2Factory::new(UNISWAP_V2_FACTORY, &provider);
    let pair_address = Address::from(
        factory
            .getPair(from_token.address, to_token.address)
            .call()
            .await
            .rpc_err()?
            .0,
    );

    if pair_address == Address::ZERO {
        state.last_message = Some("No liquidity pool for this pair".into());
        return Ok(());
    }

    // Get reserves
    let pair = IUniswapV2Pair::new(pair_address, &provider);
    let reserves = pair.getReserves().call().await.rpc_err()?;
    let (reserve0, reserve1) = (reserves.reserve0, reserves.reserve1);

    // Determine which reserve corresponds to which token
    // Uniswap V2 pairs always store tokens sorted by address (token0 < token1)
    let (reserve_in, reserve_out) = if from_token.address < to_token.address {
        (reserve0, reserve1)
    } else {
        (reserve1, reserve0)
    };

    let reserve_in: f64 = reserve_in.into();
    let reserve_out: f64 = reserve_out.into();

    // Calculate output using x*y=k with 0.3% fee
    // amountOut = (amountIn * 0.997 * reserveOut) / (reserveIn + amountIn * 0.997)
    let amount_in_with_fee = amount_in * 0.997;
    let amount_out = (amount_in_with_fee * reserve_out) / (reserve_in + amount_in_with_fee);
    state.expected_output = amount_out;

    state.last_message = Some("Quote calculated".into());

    Ok(())
}

async fn handle_execute_swap(
    transport: &Transport,
    state: &mut PluginState,
) -> Result<(), RpcError> {
    state.last_message = Some("Preparing swap...".into());

    // Validate all required fields
    let Some(from_idx) = state.selected_from_token else {
        state.last_message = Some("Select from token".into());
        return Ok(());
    };

    let Some(to_idx) = state.selected_to_token else {
        state.last_message = Some("Select to token".into());
        return Ok(());
    };

    let coordinator_id = state.coordinator_id;
    let from_token = &ERC20S[from_idx];
    let to_token = &ERC20S[to_idx];

    // Parse input amount
    let amount_in = state.input_amount;
    let expected_out = state.expected_output;

    // TODO: Add slippage tolerance
    let amount_out_min = expected_out * 0.9; // 10% slippage tolerance

    // Get coordinator session
    let account_id = coordinator::GetSession
        .call_async(
            transport.clone(),
            (coordinator_id, ChainId::new_evm(CHAIN_ID), None),
        )
        .await?;

    // Get account address
    let Some(account_address) = account_id.as_evm_address() else {
        return Err(RpcError::Custom("Invalid account address".into()));
    };

    // Build swap operations
    let operations = build_swap_operations(
        account_address,
        from_token,
        to_token,
        U256::from(amount_in),
        U256::from(amount_out_min),
    )?;

    // Build EvmBundle
    let from_asset_id = AssetId {
        chain_id: ChainId::new_evm(CHAIN_ID),
        asset: AssetType::Erc20(from_token.address),
    };
    let to_asset_id = AssetId {
        chain_id: ChainId::new_evm(CHAIN_ID),
        asset: AssetType::Erc20(to_token.address),
    };

    let bundle = coordinator::EvmBundle {
        inputs: vec![(from_asset_id, U256::from(amount_in))],
        outputs: vec![to_asset_id],
        operations,
    };

    // Propose to coordinator
    coordinator::Propose
        .call_async(transport.clone(), (coordinator_id, account_id, bundle))
        .await?;

    state.last_message = Some("Swap executed successfully!".into());
    state.input_amount = 0.0;
    state.expected_output = 0.0;

    Ok(())
}

fn build_swap_operations(
    account_address: Address,
    from_token: &erc20s::ERC20,
    to_token: &erc20s::ERC20,
    amount_in: U256,
    amount_out_min: U256,
) -> Result<Vec<coordinator::EvmOperation>, RpcError> {
    let mut operations = Vec::new();

    // Operation 1: Approve Router to spend tokens
    let approve_call = ERC20::approveCall {
        spender: UNISWAP_V2_ROUTER,
        amount: amount_in,
    };

    operations.push(coordinator::EvmOperation {
        to: from_token.address,
        value: U256::ZERO,
        data: approve_call.abi_encode(),
    });

    // Operation 2: Swap tokens
    let path = vec![from_token.address, to_token.address];
    let deadline = U256::from(u64::MAX); // Far future deadline

    let swap_call = IUniswapV2Router02::swapExactTokensForTokensCall {
        amountIn: amount_in,
        amountOutMin: amount_out_min,
        path,
        to: account_address,
        deadline,
    };

    operations.push(coordinator::EvmOperation {
        to: UNISWAP_V2_ROUTER,
        value: U256::ZERO,
        data: swap_call.abi_encode(),
    });

    Ok(operations)
}

// ---------- UI Builder Function ----------

fn build_ui(state: &PluginState) -> tlock_pdk::tlock_api::component::Component {
    let mut sections = vec![
        heading("Uniswap V2 Swap"),
        text("Swap ERC20 tokens on Sepolia using Uniswap V2"),
    ];

    // Status message
    if let Some(msg) = &state.last_message {
        sections.push(text(format!("Status: {}", msg)));
    }

    // Token selection and swap form
    sections.push(heading("Select Tokens"));

    let token_options: Vec<String> = ERC20S
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}: {}", i, t.symbol))
        .collect();

    let from_selected = state
        .selected_from_token
        .map(|i| format!("{}: {}", i, ERC20S[i].symbol));
    let to_selected = state
        .selected_to_token
        .map(|i| format!("{}: {}", i, ERC20S[i].symbol));

    sections.push(form(
        "swap_form",
        vec![
            dropdown(
                "from_token",
                "From Token:",
                token_options.clone(),
                from_selected,
            ),
            dropdown("to_token", "To Token:", token_options, to_selected),
            text_input("amount", "Amount (wei)", "1500"),
            submit_input("Update Quote"),
        ],
    ));

    // Display quote if available
    let formatted_in = state.input_amount / 10f64.powi(state.input_decimals as i32);
    let formatted_out = state.expected_output / 10f64.powi(state.output_decimals as i32);

    if formatted_out > 0.0 {
        sections.push(heading("Quote"));
        sections.push(text(format!("Expected Output: {:.4}", formatted_out)));

        if formatted_in != 0.0 {
            let exchange_rate = formatted_out / formatted_in;
            sections.push(text(format!("Exchange Rate: {:.4}", exchange_rate)));
        }

        sections.push(button_input("refresh_quote", "Refresh Quote"));
        sections.push(button_input("execute_swap", "Execute Swap"));
    }

    // Display selected token info
    if let Some(from_idx) = state.selected_from_token {
        let token = &ERC20S[from_idx];
        sections.push(text(format!(
            "From: {} ({:?})",
            token.symbol, token.address
        )));
    }

    if let Some(to_idx) = state.selected_to_token {
        let token = &ERC20S[to_idx];
        sections.push(text(format!("To: {} ({:?})", token.symbol, token.address)));
    }

    container(sections)
}

// ---------- Main Entry Point ----------

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting Uniswap V2 Plugin...");

    PluginRunner::new()
        .with_method(plugin::Init, init)
        .with_method(global::Ping, ping)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .run();
}
