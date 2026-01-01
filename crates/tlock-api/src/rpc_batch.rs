use wasmi_plugin_pdk::rpc_message::{RpcError, RpcResponse};

pub trait RpcBatch {
    type Params;
    type Outputs;

    fn requests(params: Self::Params) -> Vec<(&'static str, serde_json::Value)>;

    fn decode(responses: Vec<RpcResponse>) -> Result<Self::Outputs, RpcError>;

    fn execute<T, E>(transport: T, params: Self::Params) -> Result<Self::Outputs, RpcError>
    where
        T: wasmi_plugin_pdk::transport::SyncManyTransport<E>,
        E: Into<RpcError>,
    {
        let reqs = Self::requests(params);
        let resps = transport.call_many(reqs).map_err(Into::into)?;
        Self::decode(resps)
    }
}

macro_rules! impl_rpc_batch {
    ($($ty:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($ty),*> RpcBatch for ($($ty,)*)
        where
            $($ty: crate::RpcMethod,)*
        {
            type Params = ($($ty::Params,)*);
            type Outputs = ($($ty::Output,)*);

            fn requests(params: Self::Params) -> Vec<(&'static str, serde_json::Value)> {
                // Deconstruct the tuple of params
                let ($($ty,)*) = params;
                vec![
                    $(
                        (
                            $ty::NAME,
                            serde_json::to_value($ty).unwrap_or(serde_json::Value::Null)
                        ),
                    )*
                ]
            }

            fn decode(responses: Vec<RpcResponse>) -> Result<Self::Outputs, RpcError> {
                let mut iter = responses.into_iter();
                Ok((
                    $(
                        {
                            let resp = iter.next()
                                .ok_or_else(|| RpcError::Custom("Missing response in batch".into()))?;
                            serde_json::from_value::<$ty::Output>(resp.result)
                                .map_err(|e| RpcError::Custom(format!("Deserialization Error: {}", e)))?
                        },
                    )*
                ))
            }
        }
    };
}

// Generate implementations for common tuple sizes
impl_rpc_batch!(M1);
impl_rpc_batch!(M1, M2);
impl_rpc_batch!(M1, M2, M3);
impl_rpc_batch!(M1, M2, M3, M4);
impl_rpc_batch!(M1, M2, M3, M4, M5);
impl_rpc_batch!(M1, M2, M3, M4, M5, M6);
