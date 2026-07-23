fmt:
	cargo +nightly fmt --all

clippy:
	cargo clippy --all-features --all-targets

# Run wasm integration tests against a fresh surfpool instance.
# Requires: surfpool, wasm-bindgen-test-runner, Node.
test:
	#!/usr/bin/env bash
	set -euo pipefail
	NO_DNA=1 surfpool start --ci >/dev/null 2>&1 &
	pid=$!
	trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT
	# Wait until the RPC reports healthy (up to ~10s).
	for _ in $(seq 1 100); do
		if curl -sf -X POST http://127.0.0.1:8899 \
			-H 'Content-Type: application/json' \
			-d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
			| grep -q '"result":"ok"'; then
			break
		fi
		sleep 0.1
	done
	cargo test --target wasm32-unknown-unknown --tests --all-features
	# Test with `check_address` disabled
	cargo test --target wasm32-unknown-unknown --tests --features pubsub
