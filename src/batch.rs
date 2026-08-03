use {
    crate::provider::HttpProvider,
    serde::de::DeserializeOwned,
    serde_json::Value,
    solana_rpc_client_types::request::{RpcError, RpcRequest},
};

pub struct BatchRequest {
    provider: HttpProvider,
    ids: Vec<u64>,
    requests: Vec<Value>,
}

impl BatchRequest {
    pub(crate) fn new(provider: HttpProvider) -> Self {
        Self {
            provider,
            ids: Vec::new(),
            requests: Vec::new(),
        }
    }
}
