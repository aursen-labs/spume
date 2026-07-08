//! Integration tests against a local `surfpool` validator.
//!
//! Start surfpool first (`NO_DNA=1 surfpool start --ci`), then:
//!
//! ```text
//! cargo test --target wasm32-unknown-unknown
//! ```
//!
//! The `just test` recipe wraps both steps.

#![cfg(target_arch = "wasm32")]

#[cfg(feature = "pubsub")]
use {
    futures::stream::StreamExt, solana_rpc_client_types::config::RpcTransactionLogsFilter,
    spume::WasmPubsubClient,
};
use {
    solana_address::{Address, address},
    solana_rpc_client_types::config::{CommitmentConfig, RpcContextConfig},
    spume::WasmClient,
    wasm_bindgen_test::wasm_bindgen_test,
};

const RPC_URL: &str = "http://127.0.0.1:8899";

const SYSTEM_PROGRAM: Address = address!("11111111111111111111111111111111");

#[cfg(feature = "pubsub")]
const WS_URL: &str = "ws://127.0.0.1:8900";

#[wasm_bindgen_test]
async fn http_get_health() {
    let client = WasmClient::new(RPC_URL);
    let result = client.get_health().await.expect("getHealth failed");
    assert_eq!(result, "ok");
}

#[wasm_bindgen_test]
async fn http_with_header_does_not_break_request() {
    let client = WasmClient::new(RPC_URL)
        .with_header("x-api-key", "test-key-123")
        .with_header("authorization", "Bearer some-token");
    let result = client.get_health().await.expect("getHealth failed");
    assert_eq!(result, "ok");
}

#[wasm_bindgen_test]
async fn http_max_response_size_rejects_oversized() {
    let client = WasmClient::new(RPC_URL).with_max_response_size(8);
    let err = client
        .get_health()
        .await
        .expect_err("expected size-limit rejection, got Ok");
    let msg = err.to_string();
    assert!(
        msg.contains("response body too large"),
        "unexpected error: {msg}"
    );
}

#[wasm_bindgen_test]
async fn http_max_response_size_allows_normal_request() {
    let client = WasmClient::new(RPC_URL).with_max_response_size(10 * 1024 * 1024);
    let result = client.get_health().await.expect("getHealth failed");
    assert_eq!(result, "ok");
}

#[wasm_bindgen_test]
async fn http_get_version() {
    let client = WasmClient::new(RPC_URL);
    let version = client.get_version().await.expect("getVersion failed");
    assert!(
        !version.solana_core.is_empty(),
        "expected non-empty solana_core, got {version:?}"
    );
}

#[wasm_bindgen_test]
async fn http_get_slot_advances() {
    let client = WasmClient::new(RPC_URL);
    let first = client.get_slot(None).await.expect("getSlot failed");
    // Surfpool ticks at 400ms; poll until we see a higher slot or give up after ~3s.
    for _ in 0..15 {
        gloo_timers::future::TimeoutFuture::new(250).await;
        let next = client.get_slot(None).await.expect("getSlot failed");
        if next > first {
            return;
        }
    }
    panic!("slot did not advance past {first}");
}

#[wasm_bindgen_test]
async fn http_get_latest_blockhash() {
    let client = WasmClient::new(RPC_URL);
    let resp = client
        .get_latest_blockhash(None)
        .await
        .expect("getLatestBlockhash failed");
    assert!(
        !resp.value.blockhash.is_empty(),
        "expected non-empty blockhash"
    );
    assert!(
        resp.value.last_valid_block_height > 0,
        "expected non-zero last_valid_block_height"
    );
}

#[wasm_bindgen_test]
async fn http_get_account_info_for_system_program() {
    let client = WasmClient::new(RPC_URL);
    let resp = client
        .get_account_info(&SYSTEM_PROGRAM, None)
        .await
        .expect("getAccountInfo failed");
    assert!(
        resp.value.is_some(),
        "system program account should always exist"
    );
}

#[wasm_bindgen_test]
async fn http_get_balance_for_system_program() {
    let client = WasmClient::new(RPC_URL);
    let _ = client
        .get_balance(&SYSTEM_PROGRAM, None)
        .await
        .expect("getBalance failed");
}

#[wasm_bindgen_test]
async fn http_get_blocks_with_none_end_slot() {
    let client = WasmClient::new(RPC_URL);
    let slot = client.get_slot(None).await.expect("getSlot failed");
    let start = slot.saturating_sub(5);
    let _blocks = client
        .get_blocks(
            start,
            None,
            Some(RpcContextConfig {
                commitment: Some(CommitmentConfig::finalized()),
                ..Default::default()
            }),
        )
        .await
        .expect("getBlocks with None params failed");
}

#[wasm_bindgen_test]
async fn http_get_leader_schedule_with_none_slot_and_config() {
    let client = WasmClient::new(RPC_URL);
    let _schedule = client
        .get_leader_schedule(None, None)
        .await
        .expect("getLeaderSchedule with None params failed");
}

#[wasm_bindgen_test]
async fn http_get_genesis_hash() {
    let client = WasmClient::new(RPC_URL);
    let hash = client
        .get_genesis_hash()
        .await
        .expect("getGenesisHash failed");
    assert!(!hash.is_empty(), "expected non-empty genesis hash");
}

#[cfg(feature = "pubsub")]
#[wasm_bindgen_test]
async fn ws_slot_subscribe_receives_notification() {
    let client = WasmPubsubClient::connect(WS_URL).expect("WebSocket connect failed");
    let mut sub = client.slot_subscribe().await.expect("slotSubscribe failed");
    let info = sub
        .next()
        .await
        .expect("subscription closed before any notification")
        .expect("failed to deserialize SlotInfo");
    assert!(info.slot > 0, "expected non-zero slot, got {info:?}");
}

#[cfg(feature = "pubsub")]
#[wasm_bindgen_test]
async fn ws_unsubscribe_acknowledged() {
    let client = WasmPubsubClient::connect(WS_URL).expect("WebSocket connect failed");
    let sub = client.slot_subscribe().await.expect("slotSubscribe failed");
    let ack = sub.unsubscribe().await.expect("slotUnsubscribe failed");
    assert!(ack, "server should ack unsubscribe");
}

#[cfg(feature = "pubsub")]
#[wasm_bindgen_test]
async fn ws_subscribe_drop_does_not_panic() {
    // Drop the subscription without explicit unsubscribe — the Drop impl must
    // fire-and-forget the unsubscribe RPC and return cleanly.
    let client = WasmPubsubClient::connect(WS_URL).expect("WebSocket connect failed");
    let sub = client.slot_subscribe().await.expect("slotSubscribe failed");
    drop(sub);
    // Following requests on the same connection should still work.
    let sub2 = client
        .slot_subscribe()
        .await
        .expect("second subscribe failed");
    assert!(sub2.unsubscribe().await.expect("unsubscribe failed"));
}

#[cfg(feature = "pubsub")]
#[wasm_bindgen_test]
async fn ws_logs_subscribe_unsubscribe() {
    let client = WasmPubsubClient::connect(WS_URL).expect("WebSocket connect failed");
    let sub = client
        .logs_subscribe(RpcTransactionLogsFilter::All, None)
        .await
        .expect("logsSubscribe failed");
    assert!(
        sub.unsubscribe().await.expect("logsUnsubscribe failed"),
        "server should ack logsUnsubscribe"
    );
}

#[cfg(feature = "pubsub")]
#[wasm_bindgen_test]
async fn ws_is_connected_returns_true_when_open() {
    let client = WasmPubsubClient::connect(WS_URL).expect("WebSocket connect failed");

    // The connection should be active immediately upon successful connect
    assert!(client.is_connected(), "client should report as connected");
}
