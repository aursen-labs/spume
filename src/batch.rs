use {
    crate::{
        WasmClient,
        provider::{HttpProvider, parse_rpc_error},
    },
    serde::de::DeserializeOwned,
    serde_json::Value,
    solana_rpc_client_types::request::{RpcError, RpcRequest},
    std::{collections::HashMap, marker::PhantomData},
};

type RpcResult<T> = Result<T, Box<RpcError>>;

/// Describes a single JSON-RPC method.
///
/// Implementing this trait lets a type be passed to [`WasmClient::call`] for a
/// one-shot request, or to [`BatchRequest::add`] to be bundled into a batch.
/// `goby` provides an impl for every built-in method; downstream crates can
/// implement it for custom RPC methods the cluster exposes.
pub trait RpcMethod {
    /// The decoded `result` field returned by the server for this method.
    type Output: DeserializeOwned;

    /// The JSON-RPC method identifier.
    fn request(&self) -> RpcRequest;

    /// The positional parameters as a JSON array (`[...]`).
    fn params(&self) -> Value;
}

/// A pending JSON-RPC batch.
///
/// Accumulates requests via [`add`](Self::add) (or the typed convenience
/// methods that mirror [`WasmClient`]), then dispatches them in a single HTTP
/// POST with [`send`](Self::send). Construct one with [`WasmClient::batch`].
///
/// ```no_run
/// # use spume::WasmClient;
/// # async fn run(client: WasmClient) -> Result<(), Box<solana_rpc_client_types::request::RpcError>> {
/// let mut batch = client.batch();
/// let slot = batch.get_slot(None);
/// let version = batch.get_version();
/// let res = batch.send().await?;
/// let _ = res.get(slot)?;
/// let _ = res.get(version)?;
/// # Ok(()) }
/// ```
pub struct BatchRequest {
    provider: HttpProvider,
    requests: Vec<Value>,
}

impl BatchRequest {
    pub(crate) fn new(provider: HttpProvider) -> Self {
        Self {
            provider,
            requests: Vec::new(),
        }
    }

    /// Queue an arbitrary [`RpcMethod`] and get back a typed handle.
    ///
    /// The handle can later be passed to [`BatchResponse::get`] to retrieve the
    /// decoded result for this specific request.
    pub fn add<M: RpcMethod>(&mut self, method: M) -> BatchHandle<M::Output> {
        let id = self.provider.next_id();
        let req = method.request().build_request_json(id, method.params());
        self.requests.push(req);
        BatchHandle {
            id,
            _marker: PhantomData,
        }
    }

    /// Number of requests currently queued in this batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Whether the batch has no requests queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Send the batch as one HTTP request and collect the per-method responses.
    ///
    /// The outer `Err` covers transport/timeout/parse failures that took down
    /// the whole batch; per-method JSON-RPC errors surface when you call
    /// [`BatchResponse::get`] for that request's handle.
    pub async fn send(self) -> RpcResult<BatchResponse> {
        if self.requests.is_empty() {
            return Ok(BatchResponse {
                entries: HashMap::new(),
            });
        }
        let array = self.provider.batch_send(self.requests).await?;
        let mut entries = HashMap::with_capacity(array.len());
        for entry in array {
            if let Some(id) = entry.get("id").and_then(Value::as_u64) {
                entries.insert(id, entry);
            }
        }
        Ok(BatchResponse { entries })
    }
}

/// Typed handle returned by [`BatchRequest::add`] (or one of the convenience
/// methods on [`BatchRequest`]).
///
/// Carries the JSON-RPC request id and the type the corresponding `result`
/// should decode to, so [`BatchResponse::get`] can return the right type
/// without a turbofish at the call site.
pub struct BatchHandle<R> {
    id: u64,
    _marker: PhantomData<fn() -> R>,
}

impl<R> BatchHandle<R> {
    /// JSON-RPC request id assigned to this handle.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl<R> Copy for BatchHandle<R> {}
impl<R> Clone for BatchHandle<R> {
    fn clone(&self) -> Self {
        *self
    }
}

/// The decoded responses for a [`BatchRequest`].
///
/// Stores the raw JSON-RPC response objects keyed by request id; each call to
/// [`get`](Self::get) decodes one of them on demand.
pub struct BatchResponse {
    entries: HashMap<u64, Value>,
}

impl BatchResponse {
    /// Decode the response for a single request.
    pub fn get<R: DeserializeOwned>(&self, handle: BatchHandle<R>) -> RpcResult<R> {
        let entry = self.entries.get(&handle.id).ok_or_else(|| {
            Box::new(RpcError::RpcRequestError(format!(
                "missing response for request id {}",
                handle.id
            )))
        })?;
        if let Some(error) = entry.get("error").filter(|e| !e.is_null()) {
            return Err(parse_rpc_error(error));
        }
        let result = entry
            .get("result")
            .ok_or_else(|| Box::new(RpcError::ParseError("missing result field".to_string())))?;
        serde_json::from_value(result.clone())
            .map_err(|err| Box::new(RpcError::ParseError(err.to_string())))
    }

    /// Number of response entries returned by the server.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the response is empty (no entries returned).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl WasmClient {
    /// Open a new batch builder backed by this client's transport.
    ///
    /// Requests added to the returned [`BatchRequest`] are sent as a single
    /// HTTP POST when [`BatchRequest::send`] is awaited.
    #[must_use]
    pub fn batch(&self) -> BatchRequest {
        BatchRequest::new(self.provider.clone())
    }

    /// Dispatch a single [`RpcMethod`] over HTTP.
    ///
    /// This is the generic counterpart to the typed convenience methods on
    /// [`WasmClient`]; use it for custom methods or when you want to route a
    /// request value through the standard pipeline.
    pub async fn call<M: RpcMethod>(&self, method: M) -> RpcResult<M::Output> {
        self.provider.send(method.request(), method.params()).await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::{GetSlot, GetVersion},
        serde_json::json,
        wasm_bindgen_test::wasm_bindgen_test,
    };

    fn make_batch() -> BatchRequest {
        BatchRequest::new(HttpProvider::new("http://localhost"))
    }

    fn fake_response(entries: Vec<Value>) -> BatchResponse {
        let mut map = HashMap::with_capacity(entries.len());
        for entry in entries {
            let id = entry
                .get("id")
                .and_then(Value::as_u64)
                .expect("test fixture missing id");
            map.insert(id, entry);
        }
        BatchResponse { entries: map }
    }

    #[wasm_bindgen_test]
    fn add_returns_handles_with_monotonic_ids() {
        let mut batch = make_batch();
        let a = batch.add(GetVersion);
        let b = batch.add(GetSlot { config: None });
        let c = batch.add(GetVersion);
        assert_eq!(batch.len(), 3);
        assert!(!batch.is_empty());
        assert_ne!(a.id(), b.id());
        assert_ne!(b.id(), c.id());
        assert_ne!(a.id(), c.id());
    }

    #[wasm_bindgen_test]
    fn typed_wrapper_and_generic_add_share_id_space() {
        // Calling the typed wrapper (`batch.get_slot`) and the generic
        // `batch.add(GetSlot { .. })` must allocate ids from the same counter
        // — otherwise the response dispatch would lose entries.
        let mut batch = make_batch();
        let typed = batch.get_slot(None);
        let generic = batch.add(GetSlot { config: None });
        assert_ne!(typed.id(), generic.id());
    }

    #[wasm_bindgen_test]
    fn get_decodes_typed_result() {
        let mut batch = make_batch();
        let slot = batch.get_slot(None);
        let version = batch.add(GetVersion);

        let response = fake_response(vec![
            json!({ "jsonrpc": "2.0", "id": slot.id(), "result": 42 }),
            json!({
                "jsonrpc": "2.0",
                "id": version.id(),
                "result": { "solana-core": "1.18.0", "feature-set": 0u32 },
            }),
        ]);

        assert_eq!(response.get(slot).expect("slot"), 42);
        let v = response.get(version).expect("version");
        assert_eq!(v.solana_core, "1.18.0");
    }

    #[wasm_bindgen_test]
    fn get_surfaces_per_method_rpc_error() {
        let mut batch = make_batch();
        let slot = batch.get_slot(None);

        let response = fake_response(vec![json!({
            "jsonrpc": "2.0",
            "id": slot.id(),
            "error": { "code": -32000, "message": "boom" },
        })]);

        let err = response.get(slot).expect_err("expected per-method error");
        match *err {
            RpcError::RpcResponseError {
                code, ref message, ..
            } => {
                assert_eq!(code, -32000);
                assert_eq!(message, "boom");
            }
            ref other => panic!("expected RpcResponseError, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn get_returns_error_for_unknown_id() {
        let mut batch = make_batch();
        let handle = batch.get_slot(None);
        let response = fake_response(vec![]);
        let err = response
            .get(handle)
            .expect_err("expected missing-response error");
        let msg = err.to_string();
        assert!(
            msg.contains("missing response for request id"),
            "unexpected error message: {msg}"
        );
    }

    #[wasm_bindgen_test]
    fn get_returns_parse_error_on_type_mismatch() {
        // Server returned a result that doesn't fit the handle's typed Output.
        let mut batch = make_batch();
        let slot = batch.get_slot(None); // Output = u64

        let response = fake_response(vec![json!({
            "jsonrpc": "2.0",
            "id": slot.id(),
            "result": "not-a-u64",
        })]);

        let err = response.get(slot).expect_err("expected parse error");
        match *err {
            RpcError::ParseError(_) => {}
            ref other => panic!("expected ParseError, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn batch_handle_is_copy() {
        // Compile-time check that handles can be reused — `get` must not move them.
        let mut batch = make_batch();
        let handle = batch.get_slot(None);
        let response = fake_response(vec![
            json!({ "jsonrpc": "2.0", "id": handle.id(), "result": 7 }),
        ]);
        assert_eq!(response.get(handle).expect("first"), 7);
        assert_eq!(response.get(handle).expect("second"), 7);
    }
}
