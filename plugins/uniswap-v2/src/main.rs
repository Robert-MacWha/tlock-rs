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
            asset, button_input, container, dropdown, form, heading, submit_input, text, text_input,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Quote {
    from_token_idx: usize,
    to_token_idx: usize,
    input_amount: U256,
    expected_output: U256,
}

#[derive(Serialize, Deserialize, Debug)]
struct PluginState {
    coordinator_id: CoordinatorId,
    provider_id: EthProviderId,
    page_id: PageId,
    quote: Option<Quote>,
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
        page_id,
        quote: None,
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

    let Some(amount_str) = form_data.get("amount") else {
        error!("Amount field missing in form data");
        return Ok(());
    };

    if amount_str.is_empty() {
        state.quote = None;
        warn!("Amount is empty, cannot calculate quote");
        return Ok(());
    }

    let Ok(amount_f64) = amount_str.parse::<f64>() else {
        state.last_message = Some("Invalid amount - must be a valid number".into());
        return Ok(());
    };

    let from_token_idx = from_token
        .split(':')
        .next()
        .and_then(|idx_str| idx_str.trim().parse::<usize>().ok());
    let to_token_idx = to_token
        .split(':')
        .next()
        .and_then(|idx_str| idx_str.trim().parse::<usize>().ok());

    if let Some(from_idx) = from_token_idx && let Some(to_idx) = to_token_idx {
        let from_decimals = ERC20S[from_idx].decimals;
        let input_amount = to_units(amount_f64, from_decimals);

        state.quote = Some(Quote {
            from_token_idx: from_idx,
            to_token_idx: to_idx,
            input_amount,
            expected_output: U256::ZERO,
        });

        calculate_quote(transport, state).await?;
    } else {
        state.last_message = Some("Select both tokens to calculate quote".into());
        state.quote = None;
    }

    Ok(())
}

async fn calculate_quote(transport: &Transport, state: &mut PluginState) -> Result<(), RpcError> {
    let Some(quote) = &mut state.quote else {
        error!("Quote not initialized");
        return Ok(());
    };

    if quote.from_token_idx == quote.to_token_idx {
        state.last_message = Some("Cannot swap same token".into());
        state.quote = None;
        return Ok(());
    }

    let from_token = &ERC20S[quote.from_token_idx];
    let to_token = &ERC20S[quote.to_token_idx];

    if quote.input_amount == U256::ZERO {
        error!("Input amount is zero");
        state.quote = None;
        return Ok(());
    }

    // Get provider
    let provider = ProviderBuilder::new()
        .connect_client(AlloyBridge::new(transport.clone(), state.provider_id));

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
    let (reserve0, reserve1) = (U256::from(reserves.reserve0), U256::from(reserves.reserve1));

    // Determine which reserve corresponds to which token
    let (reserve_in, reserve_out) = if from_token.address < to_token.address {
        (reserve0, reserve1)
    } else {
        (reserve1, reserve0)
    };

    // Calculate output using x*y=k with 0.3% fee
    // amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
    let amount_in_with_fee = quote.input_amount * U256::from(997);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;
    let amount_out = numerator / denominator;

    quote.expected_output = amount_out;
    state.last_message = Some("Quote calculated".into());

    Ok(())
}

async fn handle_execute_swap(
    transport: &Transport,
    state: &mut PluginState,
) -> Result<(), RpcError> {
    state.last_message = Some("Preparing swap...".into());

    let Some(quote) = &state.quote else {
        state.last_message = Some("No quote available".into());
        return Ok(());
    };

    let coordinator_id = state.coordinator_id;
    let from_token = &ERC20S[quote.from_token_idx];
    let to_token = &ERC20S[quote.to_token_idx];

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

    // Build swap operations with 10% slippage tolerance
    let amount_in = quote.input_amount;
    let amount_out_min = quote.expected_output * U256::from(9) / U256::from(10);

    let operations = build_swap_operations(
        account_address,
        from_token,
        to_token,
        amount_in,
        amount_out_min,
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
        inputs: vec![(from_asset_id, amount_in)],
        outputs: vec![to_asset_id],
        operations,
    };

    // Propose to coordinator
    host::Notify
        .call_async(
            transport.clone(),
            (host::NotifyLevel::Info, format!("Executing swap...")),
        )
        .await?;
    let proposal = coordinator::Propose
        .call_async(transport.clone(), (coordinator_id, account_id, bundle))
        .await;
    if let Err(err) = proposal {
        state.last_message = Some(format!("Swap failed: {}", err));
        host::Notify
            .call_async(
                transport.clone(),
                (host::NotifyLevel::Error, format!("Swap failed")),
            )
            .await?;
        return Ok(());
    }

    state.last_message = Some("Swap executed".into());
    state.quote = None;

    host::Notify
        .call_async(
            transport.clone(),
            (host::NotifyLevel::Info, format!("Swap executed")),
        )
        .await?;

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
    let mut sections = vec![heading("Uniswap V2"), text("Swap ERC20s with Uniswap V2")];

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
        .quote
        .as_ref()
        .map(|q| format!("{}: {}", q.from_token_idx, ERC20S[q.from_token_idx].symbol));
    let to_selected = state
        .quote
        .as_ref()
        .map(|q| format!("{}: {}", q.to_token_idx, ERC20S[q.to_token_idx].symbol));

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
            text_input("amount", "Amount", "1.0"),
            submit_input("Update Quote"),
        ],
    ));

    // Display quote with asset balances
    if let Some(quote) = &state.quote {
        let from_token = &ERC20S[quote.from_token_idx];
        let to_token = &ERC20S[quote.to_token_idx];

        sections.push(heading("Quote"));
        sections.push(text("Input:"));
        sections.push(asset(
            AssetId::erc20(CHAIN_ID, from_token.address),
            Some(quote.input_amount),
        ));
        sections.push(text("Output:"));
        sections.push(asset(
            AssetId::erc20(CHAIN_ID, to_token.address),
            Some(quote.expected_output),
        ));
        sections.push(button_input("execute_swap", "Execute Swap"));
    }

    container(sections)
}

fn to_units(amount: f64, decimals: u8) -> U256 {
    let multiplier = 10f64.powi(decimals as i32);
    U256::from((amount * multiplier) as u128)
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
