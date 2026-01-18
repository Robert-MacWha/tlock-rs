use std::task::Poll;

use alloy::{
    eips::BlockId,
    rpc::{
        client::RpcClient,
        json_rpc::{RequestPacket, Response, ResponsePacket, ResponsePayload, SerializedRequest},
        types::BlockTransactionsKind,
    },
    transports::{TransportError, TransportErrorKind, TransportFut},
};
use serde::Deserialize;
use serde_json::value::to_raw_value;
use tlock_pdk::{
    tlock_api::{RpcMethod, entities::EthProviderId, eth},
    wasmi_plugin_pdk::transport::Transport,
};
use tower_service::Service;
use tracing::error;

use crate::eth_request::EthRequest;

mod eth_request;
mod serde_helpers;

/// An Alloy RPC bridge that routes requests through the Tlock JSON-RPC
/// transport.
///
/// Allows alloy's provider to work with the Tlock eth-provider system, making
/// it much easier to integrate alloy-based functionality in tlock plugins.
///
/// # Example
/// ```rust,ignore
/// use alloy::providers::ProviderBuilder;
/// let provider = ProviderBuilder::new().connect_client(AlloyBridge::new(transport, provider_id));
/// ```
#[derive(Clone)]
pub struct AlloyBridge {
    transport: Transport,
    provider_id: EthProviderId,
}

impl AlloyBridge {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(transport: Transport, provider_id: EthProviderId) -> RpcClient {
        let transport = AlloyBridge {
            transport,
            provider_id,
        };
        RpcClient::new(transport, false)
    }
}

impl Service<RequestPacket> for AlloyBridge {
    type Error = TransportError;
    type Future = TransportFut<'static>;
    type Response = ResponsePacket;

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        let transport = self.transport.clone();
        let provider_id = self.provider_id;

        Box::pin(async move {
            let reqs = match req {
                RequestPacket::Single(r) => vec![r],
                RequestPacket::Batch(rs) => rs,
            };

            let mut responses = Vec::with_capacity(reqs.len());

            for req in reqs {
                let response = call_req(&req, transport.clone(), provider_id).await;
                let response = match response {
                    Ok(resp) => resp,
                    Err(e) => {
                        error!("Error handling request: req={:?}, error={:?}", req, e);
                        return Err(e);
                    }
                };
                responses.push(response);
            }

            let packet = match responses.len() {
                1 => ResponsePacket::Single(responses.into_iter().next().unwrap()),
                _ => ResponsePacket::Batch(responses),
            };

            Ok(packet)
        })
    }

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

async fn call_req(
    req: &SerializedRequest,
    transport: Transport,
    provider_id: EthProviderId,
) -> Result<Response, TransportError> {
    let id = req.id().clone();

    // Ensure params field always exists for deserialization
    let method = req.meta().method.clone();
    let default_params = serde_json::value::RawValue::from_string("[]".to_string()).unwrap();
    let params = req.params().unwrap_or(&default_params);

    let json_with_params = serde_json::json!({
        "method": method,
        "params": params
    });

    let req = EthRequest::deserialize(&json_with_params).map_err(TransportError::ser_err)?;

    let resp = match req {
        EthRequest::EthChainId(()) => {
            let resp = eth::ChainId
                .call_async(transport.clone(), provider_id)
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthBlockNumber(()) => {
            let resp = eth::BlockNumber
                .call_async(transport.clone(), provider_id)
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthCall(tx_request, block_id, state_override, block_override) => {
            let tx_request = tx_request.inner;
            let block_id = block_id.unwrap_or(BlockId::latest());
            let block_override = block_override.map(|b| *b);
            let resp = eth::Call
                .call_async(
                    transport.clone(),
                    (
                        provider_id,
                        tx_request,
                        block_id,
                        state_override,
                        block_override,
                    ),
                )
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthGetBalance(address, block_id) => {
            let block_id = block_id.unwrap_or(BlockId::latest());
            let resp = eth::GetBalance
                .call_async(transport.clone(), (provider_id, address, block_id))
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthGasPrice(_) => {
            let resp = eth::GasPrice
                .call_async(transport.clone(), provider_id)
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthEstimateGas(
            transaction_request,
            block_id,
            state_override,
            block_override,
        ) => {
            let transaction_request = transaction_request.inner;
            let block_id = block_id.unwrap_or(BlockId::latest());
            let block_override = block_override.map(|b| *b);
            let resp = eth::EstimateGas
                .call_async(
                    transport.clone(),
                    (
                        provider_id,
                        transaction_request,
                        block_id,
                        state_override,
                        block_override,
                    ),
                )
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthGetTransactionCount(address, block_id) => {
            let block_id = block_id.unwrap_or(BlockId::latest());
            let resp = eth::GetTransactionCount
                .call_async(transport.clone(), (provider_id, address, block_id))
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthSendRawTransaction(bytes) => {
            let resp = eth::SendRawTransaction
                .call_async(transport.clone(), (provider_id, bytes))
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthGetTransactionReceipt(txhash) => {
            let resp = eth::GetTransactionReceipt
                .call_async(transport.clone(), (provider_id, txhash))
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()));
            //? Map None responses to null JSON value since that's what's expected for getTransactionReceipt
            // TODO: Consider instead returning a Result<Option<Receipt>> to make this more explicit / correct.
            match resp {
                Ok(r) => serde_json::to_value(r).map_err(TransportError::ser_err)?,
                Err(_) => serde_json::to_value(None::<()>).map_err(TransportError::ser_err)?,
            }
        }
        EthRequest::EthGetBlockByNumber(block_number, full) => {
            let transactions_kind = if full {
                BlockTransactionsKind::Full
            } else {
                BlockTransactionsKind::Hashes
            };
            let resp = eth::GetBlock
                .call_async(
                    transport.clone(),
                    (provider_id, block_number.into(), transactions_kind),
                )
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthGetCodeAt(address, block_id) => {
            let block_id = block_id.unwrap_or(BlockId::latest());
            let resp = eth::GetCode
                .call_async(transport.clone(), (provider_id, address, block_id))
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthGetStorageAt(address, slot, block_id) => {
            let block_id = block_id.unwrap_or(BlockId::latest());
            let resp = eth::GetStorageAt
                .call_async(transport.clone(), (provider_id, address, slot, block_id))
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        EthRequest::EthFeeHistory(block_count, newest_block, reward_percentiles) => {
            let block_count: u64 = block_count.saturating_to();
            let resp = eth::FeeHistory
                .call_async(
                    transport.clone(),
                    (provider_id, block_count, newest_block, reward_percentiles),
                )
                .await
                .map_err(|e| TransportErrorKind::custom_str(&e.to_string()))?;
            serde_json::to_value(resp).map_err(TransportError::ser_err)?
        }
        _ => {
            return Err(TransportErrorKind::custom_str(
                format!(
                    "Unsupported request type in tlock-alloy::AlloyBridge: {:?}",
                    req
                )
                .as_str(),
            ));
        }
    };

    let response = Response {
        id,
        payload: ResponsePayload::Success(to_raw_value(&resp).map_err(TransportError::ser_err)?),
    };

    Ok(response)
}
