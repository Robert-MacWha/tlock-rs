use revm::{
    context::{BlockEnv, TxEnv, result::ExecutionResult},
    context_interface::block::BlobExcessGasAndPrice,
    primitives::{
        Address, B256, Log, TxKind, U256,
        alloy_primitives::{B64, BlockHash, Bloom, BloomInput, TxHash},
        hardfork::SpecId,
        keccak256,
    },
};
use tlock_pdk::tlock_api::alloy::{
    self,
    consensus::{
        EMPTY_OMMER_ROOT_HASH, Eip658Value, Receipt, ReceiptEnvelope, Transaction, TxEnvelope,
        TxType,
        constants::{EMPTY_RECEIPTS, EMPTY_TRANSACTIONS, EMPTY_WITHDRAWALS},
        transaction::Recovered,
    },
    rpc::{self},
};

use crate::chain::SimulatedBlock;

pub fn header_to_block_env(header: rpc::types::Header) -> BlockEnv {
    BlockEnv {
        number: U256::from(header.number),
        beneficiary: header.beneficiary,
        timestamp: U256::from(header.timestamp),
        gas_limit: header.gas_limit,
        basefee: header.base_fee_per_gas.unwrap_or_default(),
        difficulty: header.difficulty,
        prevrandao: Some(header.mix_hash),
        blob_excess_gas_and_price: header
            .excess_blob_gas
            .map(|b| BlobExcessGasAndPrice::new_with_spec(b, SpecId::default())),
    }
}

pub fn simulated_block_to_header(block: &SimulatedBlock) -> rpc::types::Header {
    block_env_to_header(&block.env, block.parent_hash, &block.results)
}

fn block_env_to_header(
    block_env: &BlockEnv,
    parent_hash: BlockHash,
    results: &[ExecutionResult],
) -> rpc::types::Header {
    let gas_used: u64 = results.iter().map(|r| r.gas_used()).sum();
    let logs_bloom =
        results
            .iter()
            .flat_map(|r| r.logs())
            .fold(Bloom::default(), |mut bloom, log| {
                accrue_log(&mut bloom, log);
                bloom
            });

    let header = alloy::consensus::Header {
        parent_hash,
        ommers_hash: EMPTY_OMMER_ROOT_HASH,
        beneficiary: block_env.beneficiary,
        state_root: B256::ZERO,
        transactions_root: EMPTY_TRANSACTIONS,
        receipts_root: EMPTY_RECEIPTS,
        logs_bloom,
        difficulty: block_env.difficulty,
        number: block_env.number.saturating_to(),
        gas_limit: block_env.gas_limit,
        gas_used: gas_used,
        timestamp: block_env.timestamp.saturating_to(),
        extra_data: Default::default(),
        mix_hash: block_env.prevrandao.unwrap_or_default(),
        nonce: B64::ZERO,
        base_fee_per_gas: Some(block_env.basefee),
        withdrawals_root: Some(EMPTY_WITHDRAWALS),
        blob_gas_used: None, // TODO: Calculate me from tx inputs
        excess_blob_gas: block_env
            .blob_excess_gas_and_price
            .map(|b| b.excess_blob_gas),
        parent_beacon_block_root: None,
        requests_hash: None,
    };

    rpc::types::Header::new(header)
}

pub fn tx_request_to_tx_env(tx_request: rpc::types::TransactionRequest) -> TxEnv {
    let mut tx_env = TxEnv::builder();
    tx_env = tx_env.tx_type(tx_request.transaction_type);
    if let Some(from) = tx_request.from {
        tx_env = tx_env.caller(from);
    }
    if let Some(gas) = tx_request.gas {
        tx_env = tx_env.gas_limit(gas);
    }
    if let Some(gas_price) = tx_request.gas_price {
        tx_env = tx_env.gas_price(gas_price);
    }
    if let Some(to) = tx_request.to {
        tx_env = tx_env.kind(to);
    }
    if let Some(value) = tx_request.value {
        tx_env = tx_env.value(value);
    }
    if let Some(input) = tx_request.input.input() {
        tx_env = tx_env.data(input.clone());
    }
    if let Some(nonce) = tx_request.nonce {
        tx_env = tx_env.nonce(nonce);
    }
    tx_env = tx_env.chain_id(tx_request.chain_id);
    if let Some(access_list) = tx_request.access_list {
        tx_env = tx_env.access_list(access_list);
    }
    tx_env = tx_env.gas_priority_fee(tx_request.max_priority_fee_per_gas);
    if let Some(blob_hashes) = tx_request.blob_versioned_hashes {
        tx_env = tx_env.blob_hashes(blob_hashes);
    }
    if let Some(max_fee_per_blob_gas) = tx_request.max_fee_per_blob_gas {
        tx_env = tx_env.max_fee_per_blob_gas(max_fee_per_blob_gas);
    }
    if let Some(authorization_list) = tx_request.authorization_list {
        tx_env = tx_env.authorization_list_signed(authorization_list);
    }

    tx_env.build_fill()
}

pub fn signed_tx_to_tx_env(tx: &TxEnvelope, from: Address) -> TxEnv {
    let mut tx_env = TxEnv::builder();

    if let Some(gas_price) = tx.gas_price() {
        tx_env = tx_env.gas_price(gas_price);
    }
    tx_env = tx_env
        .caller(from)
        .gas_limit(tx.gas_limit())
        .kind(tx.to().map(|t| TxKind::Call(t)).unwrap_or(TxKind::Create))
        .value(tx.value())
        .data(tx.input().clone())
        .nonce(tx.nonce())
        .chain_id(tx.chain_id())
        .gas_priority_fee(tx.max_priority_fee_per_gas());

    if let Some(blob_hashes) = tx.blob_versioned_hashes() {
        tx_env = tx_env.blob_hashes(blob_hashes.to_vec());
    }
    if let Some(max_fee_per_blob_gas) = tx.max_fee_per_blob_gas() {
        tx_env = tx_env.max_fee_per_blob_gas(max_fee_per_blob_gas);
    }
    if let Some(authorization_list) = tx.authorization_list() {
        tx_env = tx_env.authorization_list_signed(authorization_list.to_vec());
    }

    tx_env.build_fill()
}

pub fn result_to_tx_receipt(
    block: &SimulatedBlock,
    tx_envelope: TxEnvelope,
    from: Address,
    result: &ExecutionResult,
) -> rpc::types::TransactionReceipt {
    let tx_type = tx_envelope.tx_type().clone();
    let tx_hash = tx_envelope.hash().clone();
    let tx_info = execution_result_to_transaction_info(block, result);
    let recovered = Recovered::new_unchecked(tx_envelope, from);
    let tx = rpc::types::Transaction::from_transaction(recovered, tx_info);

    let consensus_receipt = Receipt {
        status: Eip658Value::success(),
        cumulative_gas_used: result.gas_used(),
        logs: result
            .logs()
            .iter()
            .map(|l| log_to_rpc_log(l.clone(), Some(block.env.timestamp.saturating_to()), &tx))
            .collect(),
    };
    let receipt_with_bloom = consensus_receipt.with_bloom();

    let receipt = rpc::types::TransactionReceipt {
        inner: match tx_type {
            TxType::Legacy => ReceiptEnvelope::Legacy(receipt_with_bloom),
            TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt_with_bloom),
            TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt_with_bloom),
            TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt_with_bloom),
            TxType::Eip7702 => ReceiptEnvelope::Eip7702(receipt_with_bloom),
        },
        transaction_hash: tx_hash.clone(),
        transaction_index: tx.transaction_index,
        block_hash: tx.block_hash,
        block_number: tx.block_number,
        gas_used: result.gas_used(),
        effective_gas_price: tx.gas_price().unwrap_or_default(),
        blob_gas_used: tx.blob_gas_used(),
        blob_gas_price: tx.max_fee_per_blob_gas(),
        from,
        to: tx.to(),
        contract_address: result.created_address(),
    };

    receipt
}

fn execution_result_to_transaction_info(
    block: &SimulatedBlock,
    result: &ExecutionResult,
) -> rpc::types::TransactionInfo {
    let idx = block
        .results
        .iter()
        .position(|r| r == result)
        .unwrap_or_default();

    let tx_hash = compute_tx_hash(block.env.number.saturating_to(), idx);
    let tx_info = rpc::types::TransactionInfo {
        hash: Some(tx_hash),
        index: Some(idx as u64),
        block_hash: Some(block.hash),
        block_number: Some(block.env.number.saturating_to()),
        base_fee: Some(block.env.basefee),
    };

    tx_info
}

fn log_to_rpc_log(
    log: Log,
    block_timestamp: Option<u64>,
    tx: &rpc::types::Transaction,
) -> rpc::types::Log {
    let rpc_log = rpc::types::Log {
        inner: log,
        block_hash: tx.block_hash,
        block_number: tx.block_number,
        block_timestamp: block_timestamp,
        transaction_hash: Some(tx.inner.tx_hash().clone()),
        transaction_index: tx.transaction_index,
        log_index: None, // TODO: Set me
        removed: false,
    };

    return rpc_log;
}

pub fn compute_tx_hash(block_number: u64, tx_index: usize) -> TxHash {
    keccak256(
        [
            block_number.to_be_bytes().as_slice(),
            tx_index.to_be_bytes().as_slice(),
        ]
        .concat(),
    )
}

fn accrue_log(bloom: &mut Bloom, log: &Log) {
    bloom.accrue(BloomInput::Raw(log.address.as_slice()));
    for topic in log.topics() {
        bloom.accrue(BloomInput::Raw(topic.as_slice()));
    }
}
