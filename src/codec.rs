//! Transport-free JSON-RPC codec: build a request body, interpret a response.
//!
//! Everything here is pure and dependency-light — no `wasm-bindgen`, no fetch.
//! It is what remains of the crate with `default-features = false`, so a host
//! that owns its own transport (a Crux core driving `crux_http`, a native
//! client, a test harness) can still speak Solana JSON-RPC:
//!
//! ```
//! use {
//!     solana_rpc_client_types::{request::RpcRequest, response::Response},
//!     spume::codec::{interpret_body, request_body},
//! };
//!
//! # fn main() -> Result<(), Box<solana_rpc_client_types::request::RpcError>> {
//! let body = request_body(1, RpcRequest::GetBalance, [
//!     "11111111111111111111111111111111",
//! ])?;
//! // …send `body` however you like, then:
//! # let (bytes, status) = (br#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":42}}"#, 200);
//! let balance: Response<u64> = interpret_body(bytes, status)?;
//! assert_eq!(balance.value, 42);
//! # Ok(()) }
//! ```

use {
    serde::{Deserialize, Serialize, de::DeserializeOwned},
    serde_json::Value,
    solana_rpc_client_types::request::{RpcError, RpcRequest, RpcResponseErrorData},
    std::marker::PhantomData,
};

/// One RPC call: the method, its params, and the type its result deserializes
/// into — built but not sent.
///
/// Produced by the builders in [`crate::rpc`], which carry the same signatures
/// and doc comments as the [`WasmClient`](crate::WasmClient) methods, so no
/// caller has to hand-assemble a `params` array or remember which type an RPC
/// answers with:
///
/// ```
/// # fn main() -> Result<(), Box<solana_rpc_client_types::request::RpcError>> {
/// let call = spume::rpc::get_balance("11111111111111111111111111111111", None)?;
///
/// let body = call.body(1);
/// # let (bytes, status) = (br#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":42}}"#, 200);
/// // …send `body`, then hand the bytes back to the same call:
/// let balance = call.parse(bytes, status)?; // Response<u64>
/// assert_eq!(balance.value, 42);
/// # Ok(()) }
/// ```
#[derive(Clone, Debug)]
pub struct Call<R> {
    request: RpcRequest,
    params: Value,
    // `fn() -> R` so `Call<R>` stays `Send`/`Sync` whatever `R` is.
    result: PhantomData<fn() -> R>,
}

impl<R: DeserializeOwned> Call<R> {
    #[must_use]
    pub fn new(request: RpcRequest, params: Value) -> Self {
        Self {
            request,
            params,
            result: PhantomData,
        }
    }

    /// The JSON-RPC method name, e.g. `"getBalance"`.
    #[must_use]
    pub fn method(&self) -> &'static str {
        self.request.as_str()
    }

    /// Serialize the request body. `id` is echoed back by the server.
    ///
    /// Borrows, so the call stays available to [`parse`](Self::parse) the
    /// response it comes back with.
    #[must_use]
    pub fn body(&self, id: u64) -> String {
        // Infallible: `params` is already a `Value`.
        serde_json::to_string(&JsonRpcRequest::new(id, self.request, &self.params))
            .unwrap_or_default()
    }

    /// Interpret a response body for this call. See [`interpret_body`].
    pub fn parse(&self, body: &[u8], status: u16) -> Result<R, Box<RpcError>> {
        interpret_body(body, status)
    }
}

/// A request envelope that borrows its params.
///
/// `RpcRequest::build_request_json` takes ownership of a `Value`, so building
/// through it means deep-cloning the params tree and then walking the whole
/// envelope again to serialize it. Serializing this struct writes the params
/// in place, straight to the output string.
#[derive(Serialize)]
struct JsonRpcRequest<'a, P> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: &'a P,
}

impl<'a, P: Serialize> JsonRpcRequest<'a, P> {
    fn new(id: u64, request: RpcRequest, params: &'a P) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: request.as_str(),
            params,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct JsonRpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

/// Deserialized straight into `R` — going through `Value` first would parse the
/// body once, clone the `result` subtree, then walk it again.
#[derive(Deserialize)]
struct JsonRpcResponse<R> {
    result: Option<R>,
    error: Option<JsonRpcError>,
}

/// Serialize a JSON-RPC request body for `request` with `params`.
///
/// `id` is echoed back by the server; a caller sending one request at a time
/// over one connection can pass a constant.
pub fn request_body(
    id: u64,
    request: RpcRequest,
    params: impl Serialize,
) -> Result<String, Box<RpcError>> {
    serde_json::to_string(&JsonRpcRequest::new(id, request, &params))
        .map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))
}

/// Turn a response body into `R`, or into the most specific error available.
///
/// A JSON-RPC `error` is surfaced whatever the status carrying it, but a
/// `result` only counts on a 2xx — a gateway can return `500` with a body that
/// happens to parse, and that is not a successful call.
pub fn interpret_body<R: DeserializeOwned>(body: &[u8], status: u16) -> Result<R, Box<RpcError>> {
    let is_success = (200..300).contains(&status);
    let http_error = || {
        Box::new(RpcError::RpcRequestError(format!(
            "HTTP {status}: {}",
            String::from_utf8_lossy(body)
        )))
    };

    match serde_json::from_slice::<JsonRpcResponse<R>>(body) {
        Ok(JsonRpcResponse {
            error: Some(error), ..
        }) => Err(Box::new(error.into_rpc_error())),
        Ok(JsonRpcResponse {
            result: Some(result),
            ..
        }) if is_success => Ok(result),
        // No `result` and no `error`: only legal when the result itself is
        // `null`, which some methods do return.
        Ok(JsonRpcResponse { result: None, .. }) if is_success => {
            serde_json::from_value(Value::Null)
                .map_err(|_| Box::new(RpcError::ParseError("result".to_string())))
        }
        Err(err) if is_success => Err(Box::new(RpcError::ParseError(err.to_string()))),
        _ => Err(http_error()),
    }
}

// HTTP responses deserialize the error inline; only pubsub still routes an
// already-parsed `Value` through here.
#[cfg(feature = "pubsub")]
pub(crate) fn parse_rpc_error(error: Value) -> Box<RpcError> {
    Box::new(
        serde_json::from_value::<JsonRpcError>(error)
            .map(JsonRpcError::into_rpc_error)
            .unwrap_or_else(|err| RpcError::ParseError(err.to_string())),
    )
}

impl JsonRpcError {
    pub(crate) fn into_rpc_error(self) -> RpcError {
        let data = self.rpc_response_error_data();

        RpcError::RpcResponseError {
            code: self.code,
            message: self.message,
            data,
        }
    }

    fn rpc_response_error_data(&self) -> RpcResponseErrorData {
        match self.data.as_ref() {
            Some(Value::Object(data)) => data
                .get("numSlotsBehind")
                .and_then(Value::as_u64)
                .map(|num_slots_behind| RpcResponseErrorData::NodeUnhealthy {
                    num_slots_behind: Some(num_slots_behind),
                })
                .unwrap_or(RpcResponseErrorData::Empty),
            _ => RpcResponseErrorData::Empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn request_body_carries_method_and_params() {
        let body = request_body(7, RpcRequest::GetBalance, ["addr"]).expect("serializable");
        let parsed: Value = serde_json::from_str(&body).expect("valid json");

        assert_eq!(parsed["id"], 7);
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "getBalance");
        assert_eq!(parsed["params"][0], "addr");
    }

    #[test]
    fn a_call_body_matches_the_free_function() {
        // Both envelopes borrow their params; they must still agree byte for byte.
        let params = json!(["addr", null]);
        let call: Call<u64> = Call::new(RpcRequest::GetBalance, params.clone());

        assert_eq!(
            call.body(7),
            request_body(7, RpcRequest::GetBalance, params).expect("serializable")
        );
    }

    #[test]
    fn result_only_counts_on_a_2xx() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":"ok"}"#;

        assert_eq!(interpret_body::<String>(body, 200).expect("200"), "ok");

        // A gateway can answer 500 with a body that happens to parse.
        for status in [429, 500, 502] {
            let err = interpret_body::<String>(body, status)
                .expect_err("non-2xx result should not be Ok");
            assert!(
                err.to_string().contains(&format!("HTTP {status}")),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn rpc_error_wins_over_the_status() {
        let body = br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"nope"}}"#;

        for status in [200, 500] {
            let err = interpret_body::<String>(body, status).expect_err("error body");
            assert!(err.to_string().contains("nope"), "unexpected error: {err}");
        }
    }

    #[test]
    fn null_result_is_ok_on_a_2xx() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":null}"#;
        assert!(
            interpret_body::<Option<String>>(body, 200)
                .expect("null result")
                .is_none()
        );
    }
}
