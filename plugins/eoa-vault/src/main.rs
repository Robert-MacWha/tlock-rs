use alloy::{
    consensus::{Signed, TxLegacy},
    eips::BlockId,
    hex::{self, ToHexExt},
    network::{TransactionBuilder, TxSigner},
    primitives::{Address, TxHash, U256, address},
    rpc::types::{TransactionInput, TransactionRequest},
    signers::local::PrivateKeySigner,
    sol,
    sol_types::SolCall,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::stderr, str::FromStr, sync::Arc};
use tlock_pdk::{
    futures::executor::block_on,
    server::ServerBuilder,
    state::{get_state, set_state},
    tlock_api::{
        RpcMethod,
        caip::{AccountId, AssetId, ChainId},
        component::{button_input, container, form, heading, submit_input, text, text_input},
        domains::Domain,
        entities::{EntityId, EthProviderId, PageId, VaultId},
        eth, global, host, page, plugin, vault,
    },
    wasmi_pdk::{
        rpc_message::RpcError,
        tracing::{error, info, warn},
        tracing_subscriber::fmt,
        transport::JsonRpcTransport,
    },
};

#[derive(Serialize, Deserialize, Default, Debug)]
struct PluginState {
    vaults: HashMap<EntityId, Vault>,
    page_id: Option<PageId>,
    eth_provider_id: Option<EthProviderId>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Vault {
    private_key: String,
    address: Address,
}

const CHAIN_ID: u64 = 11155111; // Sepolia
const ERC20S: [Address; 1] = [
    address!("0x1c7d4b196cb0c7b01d743fbc6116a902379c7238"), // USDC
];

sol! {
    function balanceOf(address owner) returns (uint256);
    function transfer(address to, uint256 amount);
}

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    info!("Calling Init on Vault Plugin");

    // ? Register the vault's page
    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    request_eth_provider(transport.clone()).await?;

    Ok(())
}

async fn ping(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<String, RpcError> {
    let provider_id = request_eth_provider(transport.clone()).await?;

    info!(
        "Pong received, querying chain ID from Eth provider: {}",
        provider_id
    );
    let chain_id = eth::ChainId.call(transport, provider_id).await?;
    Ok(format!("Pong! Connected to chain: {}", chain_id))
}

async fn get_assets(
    transport: Arc<JsonRpcTransport>,
    params: VaultId,
) -> Result<Vec<(AssetId, U256)>, RpcError> {
    let provider_id = request_eth_provider(transport.clone()).await?;

    let vault_id = params;
    info!("Received BalanceOf request for vault: {}", vault_id);

    //? Retrieve the plugin state to get the vault account ID
    let state: PluginState = get_state(transport.clone()).await;

    let vaults = state.vaults;
    let vault = vaults.get(&vault_id.into()).ok_or_else(|| {
        error!("Vault ID not found in state: {}", vault_id);
        RpcError::InvalidParams
    })?;

    info!("Querying ETH balance for address: {}", vault.address);
    let balance = eth::GetBalance
        .call(
            transport.clone(),
            (provider_id, vault.address, BlockId::latest()),
        )
        .await?;

    let mut balances = vec![(
        AssetId::new(ChainId::new_evm(CHAIN_ID), "slip44".into(), "60".into()),
        balance,
    )];

    // Fetch ERC20 balances
    for &erc20_address in ERC20S.iter() {
        let data = balanceOfCall {
            owner: vault.address,
        }
        .abi_encode();

        let balance_of_request = TransactionRequest::default()
            .to(erc20_address)
            .input(TransactionInput::from(data));

        let call_result = eth::Call
            .call(
                transport.clone(),
                (provider_id, balance_of_request, None, None),
            )
            .await?;

        let balance = balanceOfCall::abi_decode_returns(&call_result).unwrap_or_default();
        balances.push((
            AssetId::new(
                ChainId::new_evm(CHAIN_ID),
                "erc20".into(),
                erc20_address.encode_hex(),
            ),
            balance,
        ));
    }

    Ok(balances)
}

async fn get_deposit_address(
    transport: Arc<JsonRpcTransport>,
    params: (VaultId, AssetId),
) -> Result<AccountId, RpcError> {
    let (vault_id, asset_id) = params;
    info!("Received GetDepositAddress request for vault: {}", vault_id);

    if asset_id != AssetId::new(ChainId::new_evm(CHAIN_ID), "slip44".into(), "60".into()) {
        return Err(RpcError::Custom(
            "Only sepolia is supported for this plugin".into(),
        ));
    }

    //? Retrieve the plugin state to get the vault account ID
    let state: PluginState = get_state(transport.clone()).await;

    let vaults = state.vaults;
    let vault = vaults.get(&vault_id.into()).ok_or_else(|| {
        error!("Vault ID not found in state: {}", vault_id);
        RpcError::InvalidParams
    })?;

    let account_id = AccountId::new(ChainId::new_evm(CHAIN_ID), vault.address);
    Ok(account_id)
}

async fn on_deposit(
    _transport: Arc<JsonRpcTransport>,
    params: (VaultId, AssetId),
) -> Result<(), RpcError> {
    let (vault_id, asset_id) = params;
    info!(
        "Received OnDeposit notification for vault: {}, asset: {}",
        vault_id, asset_id
    );

    // Noop is fine since this is an EOA vault. But if we wanted to we could do something here like
    // log the deposit, notify an external service, forward the deposit onto another address, etc.

    Ok(())
}

async fn withdraw(
    transport: Arc<JsonRpcTransport>,
    params: (VaultId, AccountId, AssetId, U256),
) -> Result<(), RpcError> {
    let (vault_id, to_address, asset_id, amount) = params;
    info!(
        "Received Withdraw request for vault: {}, to address: {}, asset: {}, amount: {}",
        vault_id, to_address, asset_id, amount
    );

    let eth_asset_id = AssetId::new(ChainId::new_evm(CHAIN_ID), "slip44".into(), "60".into());

    let to_address = to_address.try_into_evm_address().map_err(|e| {
        error!("Failed to convert AccountId to Address: {}", e);
        RpcError::InvalidParams
    })?;

    //? Retrieve the plugin state to get the vault account ID
    let state: PluginState = get_state(transport.clone()).await;

    let vaults = state.vaults;
    let vault = vaults.get(&vault_id.into()).ok_or_else(|| {
        error!("Vault ID not found in state: {}", vault_id);
        RpcError::InvalidParams
    })?;

    let signer = PrivateKeySigner::from_str(&vault.private_key).map_err(|e| {
        error!("Failed to create signer: {}", e);
        RpcError::Custom("Failed to create signer".into())
    })?;

    let tx_hash = match asset_id {
        id if id == eth_asset_id => {
            withdraw_eth(transport.clone(), vault, signer, to_address, amount).await?
        }
        id if ERC20S.iter().any(|&erc20_address| {
            id == AssetId::new(
                ChainId::new_evm(CHAIN_ID),
                "erc20".into(),
                erc20_address.encode_hex(),
            )
        }) =>
        {
            let erc20_address = Address::from_str(id.reference()).map_err(|e| {
                error!(
                    "Failed to parse ERC20 address from AssetId reference: {}",
                    e
                );
                RpcError::InvalidParams
            })?;
            withdraw_erc20(
                transport.clone(),
                vault,
                signer,
                erc20_address,
                to_address,
                amount,
            )
            .await?
        }
        _ => {
            return Err(RpcError::Custom("Unknown asset type for withdrawal".into()));
        }
    };

    // Update UI
    info!("Withdrawal transaction sent with hash: {}", tx_hash);
    if let Some(page_id) = state.page_id {
        let component = container(vec![
            heading("Vault Component"),
            text("Withdrawal transaction sent!"),
            text(format!("Transaction hash: {}", tx_hash)),
        ]);

        host::SetInterface
            .call(transport.clone(), (page_id, component))
            .await?;
    }

    Ok(())
}

async fn withdraw_eth(
    transport: Arc<JsonRpcTransport>,
    vault: &Vault,
    signer: PrivateKeySigner,
    to: Address,
    amount: U256,
) -> Result<TxHash, RpcError> {
    info!(
        "Withdrawing ETH from vault: {}, to account: {}, amount: {}",
        vault.address, to, amount
    );

    let provider_id = request_eth_provider(transport.clone()).await?;

    let gas_price = eth::GasPrice.call(transport.clone(), provider_id).await?;
    let nonce = eth::GetTransactionCount
        .call(
            transport.clone(),
            (provider_id, vault.address, BlockId::latest()),
        )
        .await?;

    let mut tx = TransactionRequest::default()
        .with_to(to)
        .with_chain_id(CHAIN_ID)
        .with_value(amount)
        .with_gas_limit(21_000)
        .with_gas_price(gas_price)
        .with_nonce(nonce)
        .build_legacy()
        .map_err(|err| {
            error!("Failed to build transaction request: {}", err);
            RpcError::Custom("Failed to build transaction request".into())
        })?;

    let sig = signer.sign_transaction(&mut tx).await.map_err(|e| {
        error!("Failed to sign transaction: {}", e);
        RpcError::Custom("Failed to sign transaction".into())
    })?;

    let signed: Signed<TxLegacy> = Signed::new_unhashed(tx, sig);

    let mut raw = Vec::new();
    signed.rlp_encode(&mut raw);

    info!("Sending raw transaction: 0x{}", hex::encode(&raw));
    let txhash = eth::SendRawTransaction
        .call(transport.clone(), (provider_id, raw.into()))
        .await?;

    Ok(txhash)
}

async fn withdraw_erc20(
    transport: Arc<JsonRpcTransport>,
    vault: &Vault,
    signer: PrivateKeySigner,
    erc20_address: Address,
    to: Address,
    amount: U256,
) -> Result<TxHash, RpcError> {
    info!(
        "Withdrawing ERC20 from vault: {}, token: {}, to account: {}, amount: {}",
        vault.address, erc20_address, to, amount
    );

    let provider_id = request_eth_provider(transport.clone()).await?;
    let gas_price = eth::GasPrice.call(transport.clone(), provider_id).await?;
    let nonce = eth::GetTransactionCount
        .call(
            transport.clone(),
            (provider_id, vault.address, BlockId::latest()),
        )
        .await?;
    let data = transferCall { to, amount }.abi_encode();
    let mut tx = TransactionRequest::default()
        .to(erc20_address)
        .input(TransactionInput::from(data))
        .with_chain_id(CHAIN_ID)
        .with_gas_limit(100_000)
        .with_gas_price(gas_price)
        .with_nonce(nonce)
        .build_legacy()
        .map_err(|err| {
            error!("Failed to build transaction request: {}", err);
            RpcError::Custom("Failed to build transaction request".into())
        })?;

    let sig = signer.sign_transaction(&mut tx).await.map_err(|e| {
        error!("Failed to sign transaction: {}", e);
        RpcError::Custom("Failed to sign transaction".into())
    })?;

    let signed: Signed<TxLegacy> = Signed::new_unhashed(tx, sig);

    let mut raw = Vec::new();
    signed.rlp_encode(&mut raw);

    info!("Sending raw transaction: 0x{}", hex::encode(&raw));
    let txhash = eth::SendRawTransaction
        .call(transport.clone(), (provider_id, raw.into()))
        .await?;

    Ok(txhash)
}

async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    info!("OnPageLoad called for page: {}", page_id);

    let mut state: PluginState = get_state(transport.clone()).await;
    state.page_id = Some(page_id);
    set_state(transport.clone(), &state).await?;

    let component = container(vec![
        heading("Vault Component"),
        text("This is an example vault plugin. Please enter a dev private key."),
        button_input("generate_dev_key", "Generate Dev Key"),
        form(
            "private_key_form",
            vec![
                text_input("dev_private_key", "Enter your dev private key"),
                submit_input("Submit"),
            ],
        ),
    ]);

    host::SetInterface
        .call(transport.clone(), (page_id, component))
        .await?;

    Ok(())
}

async fn on_update(
    transport: Arc<JsonRpcTransport>,
    params: (PageId, page::PageEvent),
) -> Result<(), RpcError> {
    let (page_id, event) = params;
    info!("Page updated in Vault Plugin: {:?}", event);

    match event {
        page::PageEvent::ButtonClicked(button_id) if button_id == "generate_dev_key" => {
            //? Create a vault with a new random private key
            let signer = PrivateKeySigner::random();
            let private_key = signer.to_bytes();
            let private_key_hex = hex::encode(private_key);
            let address = signer.address();

            // Register the vault entity
            let entity_id = host::RegisterEntity
                .call(transport.clone(), Domain::Vault)
                .await?;

            // Save the vault ID and private key in the plugin state
            let mut state: PluginState = get_state(transport.clone()).await;
            state.vaults.insert(
                entity_id,
                Vault {
                    private_key: private_key_hex.clone(),
                    address,
                },
            );
            set_state(transport.clone(), &state).await?;

            let component = container(vec![
                heading("Vault Component"),
                text("New dev private key generated!"),
                text(format!("Your address: {}", address)),
                text(format!("Your private key: {}", private_key_hex)),
            ]);

            host::SetInterface
                .call(transport.clone(), (page_id, component))
                .await?;

            return Ok(());
        }
        page::PageEvent::FormSubmitted(form_id, form_data) if form_id == "private_key_form" => {
            //? Create a vault from the provided private key
            let Some(private_key) = form_data.get("dev_private_key") else {
                error!("Private key not found in form data");
                return Err(RpcError::Custom("Private key not found in form".into()));
            };

            let Some(private_key) = private_key.first() else {
                error!("Private key value is empty");
                return Err(RpcError::Custom("Private key value is empty".into()));
            };

            info!("Received private key: {}", private_key);

            let signer = PrivateKeySigner::from_str(private_key).map_err(|e| {
                error!("Failed to create signer: {}", e);
                RpcError::Custom("Failed to create signer".into())
            })?;

            let address = signer.address();

            // Register the vault entity
            let entity_id = host::RegisterEntity
                .call(transport.clone(), Domain::Vault)
                .await?;

            // Save the vault ID and private key in the plugin state
            let mut state: PluginState = get_state(transport.clone()).await;
            state.vaults.insert(
                entity_id,
                Vault {
                    private_key: private_key.clone(),
                    address,
                },
            );
            set_state(transport.clone(), &state).await?;

            let component = container(vec![
                heading("Vault Component"),
                text("Private key received!"),
                text(format!("Your address: {}", address)),
                text(format!("Your private key: {}", private_key)),
            ]);

            host::SetInterface
                .call(transport.clone(), (page_id, component))
                .await?;

            return Ok(());
        }
        _ => {
            warn!("Unhandled page event: {:?}", event);
        }
    }
    Ok(())
}

async fn request_eth_provider(transport: Arc<JsonRpcTransport>) -> Result<EthProviderId, RpcError> {
    let mut state: PluginState = get_state(transport.clone()).await;
    if let Some(provider_id) = state.eth_provider_id {
        return Ok(provider_id);
    }

    let chain_id: ChainId = ChainId::new_evm(CHAIN_ID);
    let provider_id = host::RequestEthProvider
        .call(transport.clone(), chain_id)
        .await?;

    state.eth_provider_id = provider_id;
    set_state(transport.clone(), &state).await?;

    if let Some(provider_id) = state.eth_provider_id {
        Ok(provider_id)
    } else {
        Err(RpcError::Custom("Failed to obtain Eth Provider".into()))
    }
}

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(reader, writer);
    let transport = Arc::new(transport);

    let plugin = ServerBuilder::new(transport.clone())
        .with_method(plugin::Init, init)
        .with_method(global::Ping, ping)
        .with_method(vault::GetAssets, get_assets)
        .with_method(vault::Withdraw, withdraw)
        .with_method(vault::GetDepositAddress, get_deposit_address)
        .with_method(vault::OnDeposit, on_deposit)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .finish();
    let plugin = Arc::new(plugin);

    block_on(async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    });
}
