use {
    crate::codec::{interpret_body, request_body},
    futures::{
        future::{Either, select},
        pin_mut,
    },
    gloo_net::http::{Method as HttpMethod, RequestBuilder, Response},
    gloo_timers::future::TimeoutFuture,
    js_sys::{Reflect, Uint8Array},
    serde::de::DeserializeOwned,
    solana_rpc_client_types::request::{RpcError, RpcRequest},
    std::{cell::Cell, rc::Rc, time::Duration},
    wasm_bindgen_futures::JsFuture,
    web_sys::{
        AbortController, ReadableStreamDefaultReader,
        wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt},
    },
};

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
        let body = request_body(self.next_id(), request, params)?;
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

    fn next_id(&self) -> u64 {
        let id = self.id.get().wrapping_add(1);
        self.id.set(id);
        id
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
