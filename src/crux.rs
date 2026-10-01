//! Crux clients. Enable `crux` with `default-features = false` for native cores.
//!
//! HTTP uses `crux_http::HttpRequest`. PubSub uses [`WebSocketRequest`], which
//! the shell must implement using its platform's WebSocket API. Neither client
//! performs I/O in the core. Construct them inside a `Command::new` closure.
//!
//! The shell owns network timeouts and download limits. HTTP bodies and socket
//! messages are also checked against a 10 MiB limit here, **after** crossing the
//! bridge; enforce download limits in the shell to bound memory use there.

use {
    crate::codec::{Call, interpret_body, request_body},
    crux_core::{Request, capability::Operation, command::CommandContext},
    crux_http::{Http, HttpError, HttpRequest, Url},
    facet::Facet,
    futures::{Stream, StreamExt, stream::BoxStream},
    serde::{Deserialize, Serialize, de::DeserializeOwned},
    serde_json::Value,
    solana_rpc_client_types::request::{RpcError, RpcRequest},
    std::{
        marker::PhantomData,
        panic::{AssertUnwindSafe, catch_unwind},
        pin::Pin,
        sync::atomic::{AtomicU64, Ordering},
        task::{Context, Poll, ready},
    },
};

type RpcResult<T> = Result<T, Box<RpcError>>;
const MAX_RESPONSE_SIZE: usize = 10 * 1024 * 1024;
static NEXT_SOCKET_ID: AtomicU64 = AtomicU64::new(1);

fn request_error(error: impl ToString) -> Box<RpcError> {
    Box::new(RpcError::RpcRequestError(error.to_string()))
}

fn parse_error(error: impl ToString) -> Box<RpcError> {
    Box::new(RpcError::ParseError(error.to_string()))
}

fn endpoint(url: &str, schemes: &[&str]) -> RpcResult<Url> {
    let url = Url::parse(url).map_err(request_error)?;
    if !schemes.contains(&url.scheme()) || url.host_str().is_none() {
        return Err(request_error("invalid endpoint scheme or host"));
    }
    Ok(url)
}

/// HTTP RPC client bound to a Crux command. Methods match `WasmClient`.
///
/// Requests become `crux_http::HttpRequest` effects; the shell supplies replies.
/// Transport timeouts must be configured in the shell.
pub struct CruxClient<Effect, Event> {
    url: String,
    ctx: CommandContext<Effect, Event>,
    headers: Vec<(String, String)>,
    max_response_size: usize,
}

impl<Effect, Event> Clone for CruxClient<Effect, Event> {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            ctx: self.ctx.clone(),
            headers: self.headers.clone(),
            max_response_size: self.max_response_size,
        }
    }
}

impl<Effect, Event> CruxClient<Effect, Event>
where
    Effect: From<Request<HttpRequest>> + Send + 'static,
    Event: Send + 'static,
{
    /// Construct a client. Invalid endpoints are reported when sending a call.
    #[must_use]
    pub fn new(url: impl ToString, ctx: CommandContext<Effect, Event>) -> Self {
        Self {
            url: url.to_string(),
            ctx,
            headers: Vec::new(),
            max_response_size: MAX_RESPONSE_SIZE,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Add a request header. Invalid names/values fail before issuing an effect.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    /// Limit bodies accepted from the shell (default 10 MiB).
    /// This does not limit the shell's download or buffer allocation.
    #[must_use]
    pub fn with_max_response_size(mut self, bytes: usize) -> Self {
        self.max_response_size = bytes;
        self
    }

    /// Send a typed call, including custom calls built with [`Call::new`].
    pub async fn send<R: DeserializeOwned>(&self, call: Call<R>) -> RpcResult<R> {
        let url = endpoint(&self.url, &["http", "https"])?;
        let mut request = Http::request(crux_http::Method::POST, url)
            .body(call.body(1))
            .content_type(crux_http::mime::APPLICATION_JSON)
            .header("accept", "application/json");
        for (key, value) in &self.headers {
            let key = crux_http::http::HeaderName::try_from(key.as_str()).map_err(request_error)?;
            // RequestBuilder::header panics on invalid values, so validate first.
            crux_http::http::HeaderValue::try_from(value.as_str()).map_err(request_error)?;
            request = request.header(key, value);
        }
        let response = request.build().into_future(self.ctx.clone()).await;
        let (body, status) = match &response {
            Ok(response) => (
                response.body().map(Vec::as_slice).unwrap_or_default(),
                response.status().as_u16(),
            ),
            Err(error @ HttpError::Http { code, .. }) => (error.body().unwrap_or_default(), *code),
            Err(error) => return Err(request_error(error)),
        };
        if body.len() > self.max_response_size {
            return Err(request_error("response body too large"));
        }
        call.parse(body, status)
    }
}

/// WebSocket operations implemented by the Crux shell.
///
/// `Open` is a streaming request: open the URL, send `message` once connected,
/// then resolve the request repeatedly with incoming text frames, in order.
/// Report connection/read/write failures as `Error` and closure as `Closed`.
/// `Send` and `Close` are fire-and-forget notifications: do not resolve them.
/// Route send failures to the corresponding `Open` stream. Close is idempotent,
/// including when received during connection setup. Stop and close the socket
/// if resolving its stream fails (the core has cancelled the command).
///
/// The shell must enforce connection/request timeouts and message size limits.
/// It must not silently reconnect: server subscription IDs would be stale.
#[derive(Facet, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[repr(C)]
pub enum WebSocketRequest {
    Open {
        id: u64,
        url: String,
        message: String,
    },
    Send {
        id: u64,
        message: String,
    },
    Close {
        id: u64,
    },
}

/// Responses to the streaming [`WebSocketRequest::Open`] operation.
#[derive(Facet, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[repr(C)]
pub enum WebSocketMessage {
    Text(String),
    Closed,
    Error(String),
}

impl Operation for WebSocketRequest {
    type Output = WebSocketMessage;
}

/// Typed PubSub client bound to a Crux command.
///
/// Construction is lazy; each subscribe method opens its own socket through
/// the shell. Dropping a subscription closes that socket. A disconnect yields
/// one error and ends the stream; create a new subscription to reconnect.
pub struct CruxPubsubClient<Effect, Event> {
    url: String,
    ctx: CommandContext<Effect, Event>,
}

impl<Effect, Event> Clone for CruxPubsubClient<Effect, Event> {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            ctx: self.ctx.clone(),
        }
    }
}

impl<Effect, Event> CruxPubsubClient<Effect, Event>
where
    Effect: From<Request<WebSocketRequest>> + Send + 'static,
    Event: Send + 'static,
{
    #[must_use]
    pub fn new(url: impl ToString, ctx: CommandContext<Effect, Event>) -> Self {
        Self {
            url: url.to_string(),
            ctx,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub(crate) async fn subscribe<T: DeserializeOwned>(
        &self,
        subscribe_method: &'static str,
        unsubscribe_method: &'static str,
        params: Value,
    ) -> RpcResult<CruxSubscription<T>> {
        endpoint(&self.url, &["ws", "wss"])?;
        let message = request_body(
            1,
            RpcRequest::Custom {
                method: subscribe_method,
            },
            params,
        )?;
        // ponytail: one socket per subscription; multiplex when connection counts matter.
        let id = NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed);
        let incoming = self
            .ctx
            .stream_from_shell(WebSocketRequest::Open {
                id,
                url: self.url.clone(),
                message,
            })
            .boxed();
        let ctx = self.ctx.clone();
        let mut subscription = CruxSubscription {
            incoming,
            socket: Socket {
                id,
                emit: Box::new(move |op| ctx.notify_shell(op)),
            },
            server_id: 0,
            unsubscribe_method,
            ended: false,
            result: PhantomData,
        };
        subscription.server_id = subscription.response(1).await?;
        Ok(subscription)
    }
}

struct Socket {
    id: u64,
    emit: Box<dyn Fn(WebSocketRequest) + Send>,
}

impl Drop for Socket {
    fn drop(&mut self) {
        // Crux 0.20 drops the effect receiver before its tasks, so notify_shell
        // can panic during Command teardown. Cleanup must be best-effort.
        let _ = catch_unwind(AssertUnwindSafe(|| {
            (self.emit)(WebSocketRequest::Close { id: self.id });
        }));
    }
}

/// A stream of typed notifications. Drop closes its socket; [`Self::unsubscribe`]
/// sends the Solana unsubscribe request and waits for its acknowledgement first.
/// Oversized notifications yield an error without ending the stream.
///
/// With `panic = "abort"`, unsubscribe before destroying the owning command.
/// Crux 0.20's closed-channel notification panic cannot be caught in that mode.
pub struct CruxSubscription<T> {
    incoming: BoxStream<'static, WebSocketMessage>,
    socket: Socket,
    server_id: u64,
    unsubscribe_method: &'static str,
    ended: bool,
    result: PhantomData<fn() -> T>,
}

impl<T> CruxSubscription<T> {
    async fn response<R: DeserializeOwned>(&mut self, id: u64) -> RpcResult<R> {
        while let Some(message) = self.incoming.next().await {
            let Some(value) = decode_message(message)? else {
                continue;
            };
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return interpret_body(&serde_json::to_vec(&value).map_err(parse_error)?, 200);
            }
        }
        Err(request_error("websocket connection closed"))
    }

    pub async fn unsubscribe(mut self) -> RpcResult<bool> {
        if self.ended {
            return Ok(true);
        }
        let message = request_body(
            2,
            RpcRequest::Custom {
                method: self.unsubscribe_method,
            },
            [self.server_id],
        )?;
        (self.socket.emit)(WebSocketRequest::Send {
            id: self.socket.id,
            message,
        });
        self.response(2).await
    }
}

// Malformed JSON is discarded; oversized messages are reported to the caller.
fn decode_message(message: WebSocketMessage) -> RpcResult<Option<Value>> {
    match message {
        WebSocketMessage::Text(text) => {
            if text.len() > MAX_RESPONSE_SIZE {
                return Err(request_error("websocket message too large"));
            }
            Ok(serde_json::from_str(&text).ok())
        }
        WebSocketMessage::Closed => Err(request_error("websocket connection closed")),
        WebSocketMessage::Error(error) => Err(request_error(error)),
    }
}

impl<T: DeserializeOwned> Stream for CruxSubscription<T> {
    type Item = RpcResult<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.ended {
            return Poll::Ready(None);
        }
        loop {
            let message = ready!(this.incoming.as_mut().poll_next(cx));
            let disconnected = !matches!(message, Some(WebSocketMessage::Text(_)));
            let mut value = match message.map(decode_message) {
                Some(Ok(Some(value))) => value,
                Some(Ok(None)) => continue,
                error => {
                    this.ended = disconnected;
                    return Poll::Ready(Some(Err(error
                        .and_then(Result::err)
                        .unwrap_or_else(|| request_error("websocket connection closed")))));
                }
            };
            if value.get("id").is_some() {
                continue;
            }
            let Some(params) = value.get_mut("params") else {
                continue;
            };
            if params.get("subscription").and_then(Value::as_u64) != Some(this.server_id) {
                continue;
            }
            let result = params
                .get_mut("result")
                .map(Value::take)
                .ok_or_else(|| parse_error("missing notification result"))
                .and_then(|result| serde_json::from_value(result).map_err(parse_error));
            return Poll::Ready(Some(result));
        }
    }
}
