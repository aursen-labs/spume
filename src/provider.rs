use {
    futures::{
        future::{Either, select},
        pin_mut,
    },
    gloo_net::http::{Method as HttpMethod, RequestBuilder, Response},
    gloo_timers::future::TimeoutFuture,
    js_sys::{Reflect, Uint8Array},
    serde::{Deserialize, de::DeserializeOwned},
    serde_json::Value,
    solana_rpc_client_types::request::{RpcError, RpcRequest, RpcResponseErrorData},
    std::{cell::Cell, rc::Rc, time::Duration},
    wasm_bindgen_futures::JsFuture,
    web_sys::{
        AbortController, ReadableStreamDefaultReader,
        wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt},
    },
};

#[derive(Deserialize)]
struct JsonRpcError {
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

/// Default cap on response body size.
const DEFAULT_MAX_RESPONSE_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct HttpProvider {
    pub(crate) url: String,
    timeout: u32,
    id: Rc<Cell<u64>>,
    headers: Vec<(String, String)>,
    max_response_size: usize,
}

impl HttpProvider {
    #[must_use]
    pub fn new(url: impl ToString) -> Self {
        Self {
            url: url.to_string(),
            timeout: 60000,
            id: Rc::new(Cell::new(0)),
            headers: Vec::new(),
            max_response_size: DEFAULT_MAX_RESPONSE_SIZE,
        }
    }
    #[must_use]
    pub fn new_with_timeout(url: impl ToString, timeout: u32) -> Self {
        Self {
            timeout,
            ..Self::new(url)
        }
    }

    /// Set how long a request waits for its response (default 60 s).
    ///
    /// Durations beyond `u32::MAX` milliseconds (~49 days) are clamped.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        self
    }

    /// Attach a custom header that will be sent with every request.
    ///
    /// Use this to authenticate with hosted RPC providers, e.g.
    /// `HttpProvider::new(url).with_header("x-api-key", "…")`.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    /// Set the maximum response body size in bytes (default 10 MiB).
    #[must_use]
    pub fn with_max_response_size(mut self, bytes: usize) -> Self {
        self.max_response_size = bytes;
        self
    }
}

impl HttpProvider {
    pub async fn send<R: DeserializeOwned>(
        &self,
        request: RpcRequest,
        params: impl serde::Serialize,
    ) -> Result<R, Box<RpcError>> {
        let params = serde_json::to_value(params)
            .map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?;
        let body = request
            .build_request_json(self.next_id(), params)
            .to_string();
        let ctrl = AbortController::new().unwrap_throw();
        let timeout_fut = TimeoutFuture::new(self.timeout);
        let mut builder = RequestBuilder::new(&self.url)
            .method(HttpMethod::POST)
            .abort_signal(Some(&ctrl.signal()))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");
        for (key, value) in &self.headers {
            builder = builder.header(key, value);
        }
        let req_fut = builder
            .body(body)
            .map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?
            .send();

        pin_mut!(timeout_fut);
        pin_mut!(req_fut);

        let response = match select(timeout_fut, req_fut).await {
            Either::Left((_, _)) => {
                ctrl.abort();
                return Err(Box::new(RpcError::RpcRequestError(format!(
                    "request timed out after {}ms",
                    self.timeout
                ))));
            }
            Either::Right((response, _)) => response,
        };

        let response =
            response.map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?;
        let status = response.status();
        let body = read_body_capped(&response, self.max_response_size).await?;
        interpret_body(&body, status)
    }

    pub(crate) async fn batch_send(
        &self,
        ids: &[u64],
        requests: Vec<Value>,
    ) -> Result<Vec<Result<Value, Box<RpcError>>>, Box<RpcError>> {
        let body = Value::Array(requests).to_string();

        let ctrl = AbortController::new().unwrap_throw();
        let timeout_fut = TimeoutFuture::new(self.timeout);
        let mut builder = RequestBuilder::new(&self.url)
            .method(HttpMethod::POST)
            .abort_signal(Some(&ctrl.signal()))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");
        for (key, value) in &self.headers {
            builder = builder.header(key, value);
        }
        let req_fut = builder
            .body(body)
            .map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?
            .send();

        pin_mut!(timeout_fut);
        pin_mut!(req_fut);

        let response = match select(timeout_fut, req_fut).await {
            Either::Left((_, _)) => {
                ctrl.abort();
                return Err(Box::new(RpcError::RpcRequestError(format!(
                    "request timed out after {}ms",
                    self.timeout
                ))));
            }
            Either::Right((response, _)) => response,
        };

        let response =
            response.map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?;
        let status =
            StatusCode::from_u16(response.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        if let Some(len) = response
            .headers()
            .get("content-length")
            .and_then(|v| v.parse::<usize>().ok())
            && len > self.max_response_size
        {
            return Err(Box::new(RpcError::RpcRequestError(format!(
                "response body too large: {len} bytes (limit: {})",
                self.max_response_size
            ))));
        }

        let text = response
            .text()
            .await
            .map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?;

        if text.len() > self.max_response_size {
            return Err(Box::new(RpcError::RpcRequestError(format!(
                "response body too large: {} bytes (limit: {})",
                text.len(),
                self.max_response_size
            ))));
        }

        let response_array: Vec<Value> = serde_json::from_str(&text)
            .map_err(|err| Box::new(RpcError::ParseError(err.to_string())))?;

        let mut by_id: std::collections::HashMap<u64, Value> =
            std::collections::HashMap::with_capacity(response_array.len());

        for entry in response_array {
            if let Some(id) = entry.get("id").and_then(Value::as_u64) {
                by_id.insert(id, entry);
            }
        }

        let results = ids
            .iter()
            .map(|id| {
                let entry = by_id.remove(id).ok_or_else(|| {
                    Box::new(RpcError::RpcRequestError(format!(
                        "Missing Response For Request ID: {id}"
                    )))
                })?;

                if let Some(error) = entry.get("error").filter(|e| !e.is_null()) {
                    return Err(parse_rpc_error(error));
                }

                entry.get("result").cloned().ok_or_else(|| {
                    Box::new(RpcError::ParseError("Missing Result Field".to_string()))
                })
            })
            .collect();

        Ok(results)
    }

    pub(crate) fn next_id(&self) -> u64 {
        let id = self.id.get().wrapping_add(1);
        self.id.set(id);
        id
    }
}

/// Turn a response body into `R`, or into the most specific error available.
///
/// A JSON-RPC `error` is surfaced whatever the status carrying it, but a
/// `result` only counts on a 2xx — a gateway can return `500` with a body that
/// happens to parse, and that is not a successful call.
fn interpret_body<R: DeserializeOwned>(body: &[u8], status: u16) -> Result<R, Box<RpcError>> {
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

/// Read the body chunk by chunk, aborting as soon as it passes `limit`.
///
/// Returns raw bytes: `serde_json` validates UTF-8 as it parses, so building a
/// `String` first would walk the whole body an extra time.
async fn read_body_capped(response: &Response, limit: usize) -> Result<Vec<u8>, Box<RpcError>> {
    let js_error =
        |err: JsValue| -> Box<RpcError> { Box::new(RpcError::RpcRequestError(format!("{err:?}"))) };

    let Some(body) = response.body() else {
        return Ok(Vec::new());
    };
    let reader: ReadableStreamDefaultReader = body.get_reader().unchecked_into();

    let mut buf: Vec<u8> = Vec::new();
    loop {
        let chunk = JsFuture::from(reader.read()).await.map_err(js_error)?;
        let done = Reflect::get(&chunk, &JsValue::from_str("done"))
            .map_err(js_error)?
            .as_bool()
            .unwrap_or(true);
        if done {
            break;
        }

        let bytes: Uint8Array = Reflect::get(&chunk, &JsValue::from_str("value"))
            .map_err(js_error)?
            .unchecked_into();
        let len = bytes.length() as usize;
        if buf.len() + len > limit {
            // Cancelling the body stream terminates the fetch, so the rest is
            // never downloaded. Awaited so the promise rejection is consumed
            // here rather than surfacing as an unhandled rejection.
            let _ = JsFuture::from(reader.cancel()).await;
            return Err(Box::new(RpcError::RpcRequestError(format!(
                "response body too large: over {limit} bytes"
            ))));
        }

        let offset = buf.len();
        buf.resize(offset + len, 0);
        bytes.copy_to(&mut buf[offset..]);
    }

    Ok(buf)
}

// HTTP responses deserialize the error inline; only pubsub still routes an
// already-parsed `Value` through here.
#[cfg(feature = "pubsub")]
pub(crate) fn parse_rpc_error(error: &Value) -> Box<RpcError> {
    Box::new(
        serde_json::from_value::<JsonRpcError>(error.clone())
            .map(JsonRpcError::into_rpc_error)
            .unwrap_or_else(|err| RpcError::ParseError(err.to_string())),
    )
}

impl JsonRpcError {
    fn into_rpc_error(self) -> RpcError {
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
    use {super::*, wasm_bindgen_test::wasm_bindgen_test};

    #[wasm_bindgen_test]
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

    #[wasm_bindgen_test]
    fn rpc_error_wins_over_the_status() {
        let body = br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"nope"}}"#;

        for status in [200, 500] {
            let err = interpret_body::<String>(body, status).expect_err("error body");
            assert!(err.to_string().contains("nope"), "unexpected error: {err}");
        }
    }

    #[wasm_bindgen_test]
    fn null_result_is_ok_on_a_2xx() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":null}"#;
        assert!(
            interpret_body::<Option<String>>(body, 200)
                .expect("null result")
                .is_none()
        );
    }
}
