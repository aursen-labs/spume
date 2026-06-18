use {
    futures::{
        future::{Either, select},
        pin_mut,
    },
    gloo_net::http::{Method as HttpMethod, RequestBuilder},
    gloo_timers::future::TimeoutFuture,
    serde::{Deserialize, de::DeserializeOwned},
    serde_json::Value,
    solana_rpc_client_types::request::{RpcError, RpcRequest, RpcResponseErrorData},
    std::{cell::Cell, rc::Rc},
    web_sys::{AbortController, wasm_bindgen::UnwrapThrowExt},
};

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    data: Option<Value>,
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
            url: url.to_string(),
            timeout,
            id: Rc::new(Cell::new(0)),
            headers: Vec::new(),
            max_response_size: DEFAULT_MAX_RESPONSE_SIZE,
        }
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
        let is_success = (200..300).contains(&status);

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

        let response_json = match serde_json::from_str::<Value>(&text) {
            Ok(response_json) => response_json,
            Err(err) if is_success => {
                return Err(Box::new(RpcError::ParseError(err.to_string())));
            }
            Err(_) => {
                return Err(Box::new(RpcError::RpcRequestError(format!(
                    "HTTP {status}: {text}"
                ))));
            }
        };

        if let Some(error) = response_json.get("error").filter(|error| !error.is_null()) {
            return Err(parse_rpc_error(error));
        }

        if is_success {
            serde_json::from_value(
                response_json
                    .get("result")
                    .cloned()
                    .ok_or_else(|| Box::new(RpcError::ParseError("result".to_string())))?,
            )
            .map_err(|err| Box::new(RpcError::ParseError(err.to_string())))
        } else {
            Err(Box::new(RpcError::RpcRequestError(format!(
                "HTTP {status}: {text}"
            ))))
        }
    }

    fn next_id(&self) -> u64 {
        let id = self.id.get().wrapping_add(1);
        self.id.set(id);
        id
    }
}

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
