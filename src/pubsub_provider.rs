use {
    crate::provider::parse_rpc_error,
    futures::{
        channel::{mpsc, oneshot},
        sink::SinkExt,
        stream::{Stream, StreamExt},
    },
    gloo_net::websocket::{Message, futures::WebSocket},
    serde::de::DeserializeOwned,
    serde_json::{Value, json},
    solana_rpc_client_types::request::RpcError,
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        fmt,
        marker::PhantomData,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll},
    },
    wasm_bindgen_futures::spawn_local,
};

type PendingMap = RefCell<HashMap<u64, oneshot::Sender<Result<Value, Box<RpcError>>>>>;
type SubsMap = RefCell<HashMap<u64, mpsc::UnboundedSender<Result<Value, Box<RpcError>>>>>;

struct PubsubInner {
    out_tx: mpsc::UnboundedSender<Message>,
    pending: PendingMap,
    subscriptions: SubsMap,
    next_id: Cell<u64>,
}

impl PubsubInner {
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
    pub fn connect(url: impl ToString) -> Result<Self, Box<RpcError>> {
        let url = url.to_string();
        let ws = WebSocket::open(&url)
            .map_err(|err| Box::new(RpcError::RpcRequestError(err.to_string())))?;
        let (mut write, mut read) = ws.split();

        let (out_tx, mut out_rx) = mpsc::unbounded::<Message>();
        let inner = Rc::new(PubsubInner {
            out_tx,
            pending: RefCell::new(HashMap::new()),
            subscriptions: RefCell::new(HashMap::new()),
            next_id: Cell::new(0),
        });

        // Writer task: drains the outbound queue into the WebSocket sink.
        spawn_local(async move {
            while let Some(msg) = out_rx.next().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Reader task: routes inbound frames to pending requests or live subscriptions.
        let reader_inner = Rc::clone(&inner);
        spawn_local(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            dispatch_message(&reader_inner, value);
                        }
                    }
                    Ok(Message::Bytes(_)) => {}
                    Err(_) => break,
                }
            }
            // Connection closed: fail any in-flight requests and every live
            // subscription with the same error so consumers can distinguish a
            // disconnect from a normal end-of-stream.
            let disconnect_err = || -> Box<RpcError> {
                Box::new(RpcError::RpcRequestError(
                    "websocket connection closed".into(),
                ))
            };
            for (_, tx) in reader_inner.pending.borrow_mut().drain() {
                let _ = tx.send(Err(disconnect_err()));
            }
            for (_, tx) in reader_inner.subscriptions.borrow_mut().drain() {
                let _ = tx.unbounded_send(Err(disconnect_err()));
            }
        });

        Ok(Self { url, inner })
    }

    /// The endpoint URL this provider was opened with.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Issue a `<x>Subscribe` request and register a notification stream that
    /// auto-unsubscribes when dropped.
    pub async fn subscribe<T: DeserializeOwned + 'static>(
        &self,
        subscribe_method: &'static str,
        unsubscribe_method: &'static str,
        params: Value,
    ) -> Result<Subscription<T>, Box<RpcError>> {
        let result = send_request(&self.inner, subscribe_method, params).await?;
        let id: u64 = serde_json::from_value(result)
            .map_err(|err| Box::new(RpcError::ParseError(err.to_string())))?;

        let (tx, rx) = mpsc::unbounded::<Result<Value, Box<RpcError>>>();
        self.inner.subscriptions.borrow_mut().insert(id, tx);

        Ok(Subscription {
            id,
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
            .finish_non_exhaustive()
    }
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
        // Writer task is gone; drop the pending entry so it doesn't leak.
        inner.pending.borrow_mut().remove(&id);
        return Err(Box::new(RpcError::RpcRequestError(
            "websocket connection closed".into(),
        )));
    }

    rx.await.map_err(|_| {
        Box::new(RpcError::RpcRequestError(
            "websocket connection closed".into(),
        ))
    })?
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
    let Some(sub_id) = params.get("subscription").and_then(Value::as_u64) else {
        return;
    };
    let result = params.get("result").cloned().unwrap_or(Value::Null);
    if let Some(sender) = inner.subscriptions.borrow().get(&sub_id) {
        let _ = sender.unbounded_send(Ok(result));
    }
}

/// A live subscription that yields notifications as a [`Stream`].
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
    /// The server-assigned subscription id.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Cancel the subscription and await the server's acknowledgement.
    pub async fn unsubscribe(mut self) -> Result<bool, Box<RpcError>> {
        self.unsubscribed = true;
        self.inner.subscriptions.borrow_mut().remove(&self.id);
        let result = send_request(&self.inner, self.unsubscribe_method, json!([self.id])).await?;
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
        self.inner.subscriptions.borrow_mut().remove(&self.id);
        let body = json!({
            "jsonrpc": "2.0",
            "id": self.inner.next_id(),
            "method": self.unsubscribe_method,
            "params": [self.id],
        })
        .to_string();
        let _ = self.inner.out_tx.unbounded_send(Message::Text(body));
    }
}
