//! Lightweight, ergonomic Solana JSON-RPC client for WebAssembly.
//!
//! Wraps the Solana JSON-RPC API over HTTP and (optionally) PubSub WebSocket.
//! Returns the canonical types from [`solana_rpc_client_types`].
//!
//! - [`WasmClient`] — HTTP RPC client.
//! - [`WasmPubsubClient`] — WebSocket PubSub client, gated behind the
//!   `pubsub` feature (off by default).
//!
//! # Example
//!
//! ```no_run
//! use spume::WasmClient;
//!
//! # async fn run() -> Result<(), Box<solana_rpc_client_types::request::RpcError>> {
//! let client = WasmClient::new("https://api.mainnet-beta.solana.com");
//! let slot = client.get_slot(None).await?;
//! # Ok(()) }
//! ```
//!
//! [`solana_rpc_client_types`]: https://crates.io/crates/solana-rpc-client-types

#![cfg_attr(docsrs, feature(doc_cfg))]

use crate::provider::HttpProvider;
#[cfg(feature = "pubsub")]
use crate::pubsub_provider::PubsubProvider;

mod methods;
pub mod provider;

#[cfg(feature = "pubsub")]
mod pubsub_methods;
#[cfg(feature = "pubsub")]
#[cfg_attr(docsrs, doc(cfg(feature = "pubsub")))]
pub mod pubsub_provider;

/// Solana JSON-RPC client over HTTP.
///
/// Cheap to construct (no connection is opened until the first request) and
/// cheap to clone (the underlying provider is reference-counted).
#[derive(Clone, Debug)]
pub struct WasmClient {
    provider: HttpProvider,
}

impl WasmClient {
    /// Construct a client.
    ///
    /// - `url` — the JSON-RPC HTTP endpoint (e.g. `https://api.mainnet-beta.solana.com`).
    pub fn new(url: impl ToString) -> Self {
        let provider = HttpProvider::new(url);
        Self { provider }
    }

    /// The endpoint URL this client was constructed with.
    pub fn url(&self) -> &str {
        &self.provider.url
    }

    /// Attach a custom header sent with every request.
    ///
    /// Convenience pass-through to [`HttpProvider::with_header`] for the
    /// common builder-style chain:
    ///
    /// ```no_run
    /// use spume::WasmClient;
    /// let client = WasmClient::new("https://rpc.example.com")
    ///     .with_header("x-api-key", "my-secret-key");
    /// ```
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.provider = self.provider.with_header(key, value);
        self
    }
}

/// Solana JSON-RPC PubSub client over a single WebSocket connection.
///
/// Opens the connection eagerly in [`connect`](Self::connect); the connection
/// stays open for the lifetime of this client. Multiple subscriptions share
/// the same socket.
#[cfg(feature = "pubsub")]
#[cfg_attr(docsrs, doc(cfg(feature = "pubsub")))]
pub struct WasmPubsubClient {
    provider: PubsubProvider,
}

#[cfg(feature = "pubsub")]
impl WasmPubsubClient {
    /// Construct a PubSub client by opening a WebSocket connection.
    ///
    /// - `url` — the JSON-RPC PubSub WebSocket endpoint
    ///   (e.g. `wss://api.mainnet-beta.solana.com`).
    pub fn connect(
        url: impl ToString,
    ) -> Result<Self, Box<solana_rpc_client_types::request::RpcError>> {
        let provider = PubsubProvider::connect(url)?;
        Ok(Self { provider })
    }

    /// The endpoint URL this client was constructed with.
    pub fn url(&self) -> &str {
        self.provider.url()
    }
}
