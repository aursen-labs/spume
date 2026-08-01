//! Rough timings against a local `surfpool` validator (`just bench`).
//!
//! ponytail: no criterion — it doesn't run on wasm32. `Date::now()` ms
//! resolution is plenty when every sample is a batch of round trips.
//!
//! Latency here is mostly surfpool, not spume. The benches that say something
//! about *this crate* are the parse-heavy ones (`token_program`,
//! `multiple_accounts`), where deserializing the body dwarfs the round trip.

#![cfg(target_arch = "wasm32")]

#[cfg(feature = "pubsub")]
use spume::WasmPubsubClient;
use {
    futures::future::join_all,
    js_sys::Date,
    solana_account_decoder_client_types::UiAccountEncoding,
    solana_address::{Address, address},
    solana_rpc_client_types::config::RpcAccountInfoConfig,
    spume::WasmClient,
    std::future::Future,
    wasm_bindgen_test::{console_log, wasm_bindgen_test},
};

const RPC_URL: &str = "http://127.0.0.1:8899";
#[cfg(feature = "pubsub")]
const WS_URL: &str = "ws://127.0.0.1:8900";

const SYSTEM_PROGRAM: Address = address!("11111111111111111111111111111111");
/// ~175 KiB of base64 account data on a fresh surfpool — the parse-heavy target.
const TOKEN_PROGRAM: Address = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Run `f` `iters` times sequentially, print total and per-call average in ms.
///
/// ponytail: no per-call min/max — a single localhost round trip is under
/// `Date::now()`'s 1ms resolution, so only the aggregate means anything.
async fn bench<F, Fut>(name: &str, iters: u32, f: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = ()>,
{
    // One warm-up call: first request pays connection setup.
    f().await;

    let start = Date::now();
    for _ in 0..iters {
        f().await;
    }
    report(name, iters, Date::now() - start);
}

fn report(name: &str, iters: u32, total: f64) {
    console_log!(
        "{name}: {iters} iters, total {total:.0}ms, avg {:.3}ms",
        total / f64::from(iters)
    );
}

fn base64_config() -> Option<RpcAccountInfoConfig> {
    Some(RpcAccountInfoConfig {
        encoding: Some(UiAccountEncoding::Base64),
        ..Default::default()
    })
}

#[wasm_bindgen_test]
async fn bench_get_slot() {
    let client = WasmClient::new(RPC_URL);
    bench("get_slot", 200, || async {
        client.get_slot(None).await.expect("getSlot failed");
    })
    .await;
}

#[wasm_bindgen_test]
async fn bench_get_account_info() {
    let client = WasmClient::new(RPC_URL);
    bench("get_account_info (~280 B)", 200, || async {
        client
            .get_account_info(&SYSTEM_PROGRAM, None)
            .await
            .expect("getAccountInfo failed");
    })
    .await;
}

/// ~175 KiB body — dominated by response read + deserialization.
#[wasm_bindgen_test]
async fn bench_get_account_info_token_program() {
    let client = WasmClient::new(RPC_URL);
    bench("get_account_info (~175 KiB)", 50, || async {
        client
            .get_account_info(&TOKEN_PROGRAM, base64_config())
            .await
            .expect("getAccountInfo failed");
    })
    .await;
}

/// ~1.7 MiB body — the deserialization path with nothing else in the way.
#[wasm_bindgen_test]
async fn bench_get_multiple_accounts() {
    let client = WasmClient::new(RPC_URL);
    let addresses = [&TOKEN_PROGRAM; 10];
    bench("get_multiple_accounts (10 × ~175 KiB)", 20, || async {
        client
            .get_multiple_accounts(&addresses, base64_config())
            .await
            .expect("getMultipleAccounts failed");
    })
    .await;
}

/// Same call count as `bench_get_slot`, all in flight at once: how much the
/// client overlaps rather than serializes.
#[wasm_bindgen_test]
async fn bench_get_slot_concurrent() {
    let client = WasmClient::new(RPC_URL);
    const ITERS: u32 = 200;

    client.get_slot(None).await.expect("warm-up failed");
    let start = Date::now();
    let results = join_all((0..ITERS).map(|_| client.get_slot(None))).await;
    let total = Date::now() - start;
    for r in results {
        r.expect("getSlot failed");
    }
    report("get_slot (200 concurrent)", ITERS, total);
}

/// Subscribe + unsubscribe round trips on one WebSocket connection — the
/// pubsub bookkeeping, not notification throughput (that's the validator's
/// slot rate).
#[cfg(feature = "pubsub")]
#[wasm_bindgen_test]
async fn bench_slot_subscribe_unsubscribe() {
    let client = WasmPubsubClient::connect(WS_URL).expect("WebSocket connect failed");
    bench("slot_subscribe + unsubscribe", 50, || async {
        let sub = client.slot_subscribe().await.expect("slotSubscribe failed");
        sub.unsubscribe().await.expect("slotUnsubscribe failed");
    })
    .await;
}
