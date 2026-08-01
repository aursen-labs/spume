use {
    crate::provider::parse_rpc_error,
    futures::{
        channel::{mpsc, oneshot},
        future::{self, Either},
        sink::SinkExt,
        stream::{SplitSink, SplitStream, Stream, StreamExt},
    },
    gloo_net::websocket::{Message, futures::WebSocket},
    gloo_timers::future::sleep,
    serde::de::DeserializeOwned,
    serde_json::{Value, json},
    solana_rpc_client_types::request::RpcError,
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        fmt,
        marker::PhantomData,
        pin::Pin,
        rc::{Rc, Weak},
        task::{Context, Poll},
        time::Duration,
    },
    wasm_bindgen_futures::spawn_local,
};

/// Reconnect backoff bounds.
const RETRY_MIN: Duration = Duration::from_millis(500);
const RETRY_MAX: Duration = Duration::from_secs(8);

/// How long a handshake may sit in `CONNECTING` before we give up and retry.
/// Browsers surface a refused connection quickly, but a black-holed host can
/// hang there indefinitely.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_POLL: Duration = Duration::from_millis(25);

type PendingMap = RefCell<HashMap<u64, oneshot::Sender<Result<Value, Box<RpcError>>>>>;
type Socket = (
    SplitSink<WebSocket, Message>,
    SplitStream<WebSocket>,
    web_sys::WebSocket,
);

/// A live subscription: the consumer's channel plus what's needed to re-issue
/// the request on a fresh connection.
struct SubEntry {
    tx: mpsc::UnboundedSender<Result<Value, Box<RpcError>>>,
    subscribe_method: &'static str,
    params: Value,
    /// `None` while disconnected or mid-resubscribe.
    server_id: Option<u64>,
}

struct PubsubInner {
    out_tx: mpsc::UnboundedSender<Message>,
    pending: PendingMap,
    /// Keyed by our own id, not the server's — the server assigns a new
    /// subscription id every time we reconnect.
    subscriptions: RefCell<HashMap<u64, SubEntry>>,
    /// Server subscription id -> our id, rebuilt on every reconnect.
    server_ids: RefCell<HashMap<u64, u64>>,
    connected: Cell<bool>,
    next_id: Cell<u64>,
}

impl PubsubInner {
    fn new(out_tx: mpsc::UnboundedSender<Message>) -> Self {
        Self {
            out_tx,
            pending: RefCell::new(HashMap::new()),
            subscriptions: RefCell::new(HashMap::new()),
            server_ids: RefCell::new(HashMap::new()),
            connected: Cell::new(true),
            next_id: Cell::new(0),
        }
    }

    fn next_id(&self) -> u64 {
        let id = self.next_id.get().wrapping_add(1);
        self.next_id.set(id);
        id
    }
}

/// JSON-RPC PubSub provider over a WebSocket connection.
#[derive(Clone)]
pub struct PubsubProvider {
    url: String,
    inner: Rc<PubsubInner>,
}

impl PubsubProvider {
    /// Open a WebSocket connection to the given URL.
    ///
    /// - `url` — the JSON-RPC PubSub WebSocket endpoint
    ///   (e.g. `wss://api.mainnet-beta.solana.com`).
    ///
    /// The connection is supervised: if the socket drops, the provider
    /// reconnects with exponential backoff (500 ms doubling to 8 s) and
    /// re-issues every live subscription, so existing [`Subscription`] streams
    /// keep yielding. Each stream sees one `Err` per disconnect before it
    /// resumes; in-flight requests fail rather than hanging, as do requests
    /// issued while the connection is down (within one retry tick).
    /// Supervision stops when the last clone of the provider is dropped.
    #[must_use = "pubsub connection result must be handled"]
    pub fn connect(url: impl ToString) -> Result<Self, Box<RpcError>> {
        let url = url.to_string();
        let socket = open_socket(&url)?;

        let (out_tx, out_rx) = mpsc::unbounded::<Message>();
        let inner = Rc::new(PubsubInner::new(out_tx));

        spawn_local(supervise(
            url.clone(),
            Rc::downgrade(&inner),
            out_rx,
            socket,
        ));

        Ok(Self { url, inner })
    }

    /// The endpoint URL this provider was opened with.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns `true` if a WebSocket connection is currently established.
    ///
    /// `false` is not terminal — the provider keeps trying to reconnect.
    pub fn is_connected(&self) -> bool {
        self.inner.connected.get()
    }

    /// Issue a `<x>Subscribe` request and register a notification stream that
    /// auto-unsubscribes when dropped.
    ///
    /// The subscription is re-issued automatically after a reconnect.
    pub async fn subscribe<T: DeserializeOwned + 'static>(
        &self,
        subscribe_method: &'static str,
        unsubscribe_method: &'static str,
        params: Value,
    ) -> Result<Subscription<T>, Box<RpcError>> {
        let server_id = request_subscription(&self.inner, subscribe_method, params.clone()).await?;

        let local_id = self.inner.next_id();
        let (tx, rx) = mpsc::unbounded::<Result<Value, Box<RpcError>>>();
        self.inner.subscriptions.borrow_mut().insert(
            local_id,
            SubEntry {
                tx,
                subscribe_method,
                params,
                server_id: Some(server_id),
            },
        );
        self.inner
            .server_ids
            .borrow_mut()
            .insert(server_id, local_id);

        Ok(Subscription {
            id: local_id,
            unsubscribe_method,
            rx,
            inner: Rc::clone(&self.inner),
            unsubscribed: false,
            _phantom: PhantomData,
        })
    }
}

impl fmt::Debug for PubsubProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PubsubProvider")
            .field("url", &self.url)
            .field("connected", &self.inner.connected.get())
            .finish_non_exhaustive()
    }
}

fn open_socket(url: &str) -> Result<Socket, Box<RpcError>> {
    let raw = web_sys::WebSocket::new(url)
        .map_err(|err| Box::new(RpcError::RpcRequestError(format!("{err:?}"))))?;
    let ws = WebSocket::try_from(raw.clone())
        .map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?;
    let (write, read) = ws.split();
    Ok((write, read, raw))
}

fn disconnect_error() -> Box<RpcError> {
    Box::new(RpcError::RpcRequestError(
        "websocket connection closed".into(),
    ))
}

/// Owns the socket lifecycle: pump frames until the connection dies, then back
/// off, reconnect and re-issue live subscriptions. Returns once the last
/// `PubsubProvider` clone is dropped.
async fn supervise(
    url: String,
    inner: Weak<PubsubInner>,
    mut out_rx: mpsc::UnboundedReceiver<Message>,
    socket: Socket,
) {
    let mut next = Some(socket);
    let mut backoff = RETRY_MIN;

    loop {
        if let Some(socket) = next.take()
            && run_session(socket, &mut out_rx, &inner, &mut backoff).await
        {
            // Provider dropped: stop supervising.
            return;
        }

        // Runs after a dead connection *and* after a failed reconnect, so
        // requests issued while down fail within one backoff tick instead of
        // hanging forever.
        let Some(strong) = inner.upgrade() else {
            return;
        };
        disconnected(&strong, &mut out_rx);
        drop(strong);

        sleep(backoff).await;
        backoff = (backoff * 2).min(RETRY_MAX);
        next = open_socket(&url).ok();
    }
}

/// Runs one connection from handshake to death. Returns `true` when the
/// provider itself is gone (stop supervising), `false` when the socket died
/// or never opened (reconnect).
async fn run_session(
    (mut write, mut read, raw): Socket,
    out_rx: &mut mpsc::UnboundedReceiver<Message>,
    inner: &Weak<PubsubInner>,
    backoff: &mut Duration,
) -> bool {
    let provider_gone = if wait_open(&raw).await {
        match inner.upgrade() {
            None => true,
            Some(strong) => {
                *backoff = RETRY_MIN;
                strong.connected.set(true);
                resubscribe(&strong);
                drop(strong);
                pump(&mut write, &mut read, out_rx, inner).await
            }
        }
    } else {
        false
    };
    close_socket(write, read, raw);
    provider_gone
}

/// Wait for the handshake to settle; `false` if it failed or timed out.
async fn wait_open(raw: &web_sys::WebSocket) -> bool {
    let mut waited = Duration::ZERO;
    while raw.ready_state() == web_sys::WebSocket::CONNECTING {
        if waited >= CONNECT_TIMEOUT {
            return false;
        }
        sleep(CONNECT_POLL).await;
        waited += CONNECT_POLL;
    }
    raw.ready_state() == web_sys::WebSocket::OPEN
}

/// Close the socket, keeping the gloo-net wrapper alive until the close event
/// lands: its `Drop` leaves its own `close` listener attached, and a JS event
/// delivered after that closure is freed throws.
fn close_socket(
    write: SplitSink<WebSocket, Message>,
    mut read: SplitStream<WebSocket>,
    raw: web_sys::WebSocket,
) {
    let _ = raw.close();
    spawn_local(async move {
        while read.next().await.is_some() {}
        drop(write);
    });
}

/// Shuttle frames both ways over one connection. Returns `true` when the
/// provider itself is gone (stop supervising), `false` when the socket died
/// (reconnect).
async fn pump(
    write: &mut SplitSink<WebSocket, Message>,
    read: &mut SplitStream<WebSocket>,
    out_rx: &mut mpsc::UnboundedReceiver<Message>,
    inner: &Weak<PubsubInner>,
) -> bool {
    loop {
        // Both `next()` futures are cancel-safe, so the loser of `select` drops
        // without losing a frame.
        match future::select(read.next(), out_rx.next()).await {
            Either::Left((Some(frame), _)) => {
                if let Ok(Message::Text(text)) = frame
                    && let Some(inner) = inner.upgrade()
                    && let Ok(value) = serde_json::from_str::<Value>(&text)
                {
                    dispatch_message(&inner, value);
                }
            }
            Either::Left((None, _)) => return false,
            Either::Right((Some(msg), _)) => {
                if write.send(msg).await.is_err() {
                    return false;
                }
            }
            Either::Right((None, _)) => return true,
        }
    }
}

/// Connection is down: fail everything in flight, keep the subscriptions
/// themselves so [`resubscribe`] can restore them.
fn disconnected(inner: &Rc<PubsubInner>, out_rx: &mut mpsc::UnboundedReceiver<Message>) {
    let was_connected = inner.connected.replace(false);

    // Frames queued while the socket was down would reach the server with
    // nobody waiting for the response — drop them.
    while out_rx.try_recv().is_ok() {}

    for (_, tx) in inner.pending.borrow_mut().drain() {
        let _ = tx.send(Err(disconnect_error()));
    }
    inner.server_ids.borrow_mut().clear();
    for entry in inner.subscriptions.borrow_mut().values_mut() {
        entry.server_id = None;
        // One error per disconnect, not per retry tick.
        if was_connected {
            let _ = entry.tx.unbounded_send(Err(disconnect_error()));
        }
    }
}

/// Re-issue every live subscription on a fresh connection and remap the
/// server-assigned ids.
fn resubscribe(inner: &Rc<PubsubInner>) {
    let live: Vec<(u64, &'static str, Value)> = inner
        .subscriptions
        .borrow()
        .iter()
        .map(|(local_id, entry)| (*local_id, entry.subscribe_method, entry.params.clone()))
        .collect();
    if live.is_empty() {
        return;
    }

    let inner = Rc::clone(inner);
    spawn_local(async move {
        for (local_id, method, params) in live {
            let result = request_subscription(&inner, method, params).await;
            let mut subscriptions = inner.subscriptions.borrow_mut();
            // Consumer may have dropped the subscription while we waited.
            let Some(entry) = subscriptions.get_mut(&local_id) else {
                continue;
            };
            match result {
                Ok(server_id) => {
                    entry.server_id = Some(server_id);
                    inner.server_ids.borrow_mut().insert(server_id, local_id);
                }
                Err(err) => {
                    let _ = entry.tx.unbounded_send(Err(err));
                }
            }
        }
    });
}

async fn request_subscription(
    inner: &Rc<PubsubInner>,
    method: &str,
    params: Value,
) -> Result<u64, Box<RpcError>> {
    let result = send_request(inner, method, params).await?;
    serde_json::from_value(result).map_err(|err| Box::new(RpcError::ParseError(err.to_string())))
}

async fn send_request(
    inner: &Rc<PubsubInner>,
    method: &str,
    params: Value,
) -> Result<Value, Box<RpcError>> {
    let id = inner.next_id();
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
    .to_string();

    let (tx, rx) = oneshot::channel::<Result<Value, Box<RpcError>>>();
    inner.pending.borrow_mut().insert(id, tx);

    if inner.out_tx.unbounded_send(Message::Text(body)).is_err() {
        // Supervisor is gone; drop the pending entry so it doesn't leak.
        inner.pending.borrow_mut().remove(&id);
        return Err(disconnect_error());
    }

    rx.await.map_err(|_| disconnect_error())?
}

/// Remove a subscription from the dispatcher; returns the server-side id if
/// one was still registered (`None` while disconnected or mid-resubscribe).
fn remove_subscription(inner: &PubsubInner, local_id: u64) -> Option<u64> {
    let server_id = inner
        .subscriptions
        .borrow_mut()
        .remove(&local_id)?
        .server_id?;
    inner.server_ids.borrow_mut().remove(&server_id);
    Some(server_id)
}

// Frames with an `id` are responses to our requests; frames with `params.subscription`
// are server-pushed notifications.
fn dispatch_message(inner: &Rc<PubsubInner>, value: Value) {
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        if let Some(tx) = inner.pending.borrow_mut().remove(&id) {
            let response = match value.get("error").filter(|err| !err.is_null()) {
                Some(error) => Err(parse_rpc_error(error)),
                None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
            };
            let _ = tx.send(response);
        }
        return;
    }

    let Some(params) = value.get("params") else {
        return;
    };
    let Some(server_id) = params.get("subscription").and_then(Value::as_u64) else {
        return;
    };
    let Some(local_id) = inner.server_ids.borrow().get(&server_id).copied() else {
        return;
    };
    let result = params.get("result").cloned().unwrap_or(Value::Null);
    if let Some(entry) = inner.subscriptions.borrow().get(&local_id) {
        let _ = entry.tx.unbounded_send(Ok(result));
    }
}

/// A live subscription that yields notifications as a [`Stream`].
///
/// The stream survives reconnects: on a dropped connection it yields one `Err`,
/// then resumes once the provider has re-subscribed.
///
/// Dropping a `Subscription` removes it from the dispatcher and best-effort
/// sends the matching `<x>Unsubscribe` over the wire. Use [`Subscription::unsubscribe`]
/// to await the server's acknowledgement instead.
pub struct Subscription<T> {
    id: u64,
    unsubscribe_method: &'static str,
    rx: mpsc::UnboundedReceiver<Result<Value, Box<RpcError>>>,
    inner: Rc<PubsubInner>,
    unsubscribed: bool,
    _phantom: PhantomData<fn() -> T>,
}

impl<T> Subscription<T> {
    /// This subscription's client-side id.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Cancel the subscription and await the server's acknowledgement.
    #[must_use = "unsubscription result must be handled to ensure server acknowledged"]
    pub async fn unsubscribe(mut self) -> Result<bool, Box<RpcError>> {
        self.unsubscribed = true;

        // Not registered server-side (disconnected, or mid-resubscribe): there
        // is nothing left to cancel.
        let Some(server_id) = remove_subscription(&self.inner, self.id) else {
            return Ok(true);
        };

        let result = send_request(&self.inner, self.unsubscribe_method, json!([server_id])).await?;
        serde_json::from_value(result)
            .map_err(|err| Box::new(RpcError::ParseError(err.to_string())))
    }
}

impl<T> fmt::Debug for Subscription<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Subscription")
            .field("id", &self.id)
            .field("unsubscribe_method", &self.unsubscribe_method)
            .finish_non_exhaustive()
    }
}

impl<T: DeserializeOwned> Stream for Subscription<T> {
    type Item = Result<T, Box<RpcError>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.rx).poll_next(cx) {
            Poll::Ready(Some(Ok(value))) => Poll::Ready(Some(
                serde_json::from_value(value)
                    .map_err(|err| Box::new(RpcError::ParseError(err.to_string()))),
            )),
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        if self.unsubscribed {
            return;
        }
        let Some(server_id) = remove_subscription(&self.inner, self.id) else {
            return;
        };

        let body = json!({
            "jsonrpc": "2.0",
            "id": self.inner.next_id(),
            "method": self.unsubscribe_method,
            "params": [server_id],
        })
        .to_string();
        let _ = self.inner.out_tx.unbounded_send(Message::Text(body));
    }
}

// The reconnect paths that need a real server bounce can't be driven from the
// wasm test harness; this covers the bookkeeping they depend on.
#[cfg(test)]
mod tests {
    use {super::*, wasm_bindgen_test::wasm_bindgen_test};

    #[wasm_bindgen_test]
    fn disconnect_keeps_subscriptions_and_fails_requests() {
        let (out_tx, mut out_rx) = mpsc::unbounded::<Message>();
        let inner = Rc::new(PubsubInner::new(out_tx));

        let (notify_tx, mut notify_rx) = mpsc::unbounded();
        inner.subscriptions.borrow_mut().insert(
            1,
            SubEntry {
                tx: notify_tx,
                subscribe_method: "slotSubscribe",
                params: json!([]),
                server_id: Some(42),
            },
        );
        inner.server_ids.borrow_mut().insert(42, 1);

        let (request_tx, mut request_rx) = oneshot::channel();
        inner.pending.borrow_mut().insert(7, request_tx);
        inner
            .out_tx
            .unbounded_send(Message::Text("queued".into()))
            .expect("send failed");

        disconnected(&inner, &mut out_rx);

        assert!(!inner.connected.get());
        // The subscription survives for `resubscribe`, minus its stale server id.
        assert_eq!(inner.subscriptions.borrow().len(), 1);
        assert!(inner.subscriptions.borrow()[&1].server_id.is_none());
        assert!(inner.server_ids.borrow().is_empty());
        // In-flight request fails instead of hanging, queued frame is dropped.
        assert!(inner.pending.borrow().is_empty());
        assert!(request_rx.try_recv().expect("sender dropped").is_some());
        assert!(out_rx.try_recv().is_err(), "queued frame should be dropped");
        // Consumer is told once.
        assert!(notify_rx.try_recv().is_ok(), "expected a disconnect error");
        assert!(notify_rx.try_recv().is_err(), "expected exactly one error");
    }
}
