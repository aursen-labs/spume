# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added automatic reconnection to the pubsub client, re-issuing live subscriptions so their streams survive a dropped connection ([#37](https://github.com/aursen-labs/spume/pull/37)).
- Added `WasmPubsubClient::with_request_timeout`, capping how long a pubsub request waits for its response (default 60 s) so a socket that stays open but never answers can no longer hang the caller ([#39](https://github.com/aursen-labs/spume/pull/39)).
- Added `WasmClient::with_timeout` and `HttpProvider::with_timeout`, so the request timeout is reachable from the client and settable builder-style rather than only through `HttpProvider::new_with_timeout` ([#40](https://github.com/aursen-labs/spume/pull/40)).
- Added `Clone` and `Debug` for `WasmPubsubClient`; clones share the one connection, matching `PubsubProvider` ([#40](https://github.com/aursen-labs/spume/pull/40)).

### Changed

- Subscription streams now yield one `Err` per disconnect and resume instead of ending, and `Subscription::id` returns a stable client-side id rather than the server's ([#37](https://github.com/aursen-labs/spume/pull/37)).

### Fixed

- `with_max_response_size` now streams the response body and cancels the transfer once the cap is passed, instead of buffering the whole body before rejecting it ([#38](https://github.com/aursen-labs/spume/pull/38)).

## [0.3.1] - 2026-08-01

### Changed

- Constrained the Solana dependencies to patch ranges so newer releases can't break the `wasm32-unknown-unknown` build, and committed `Cargo.lock` ([#35](https://github.com/aursen-labs/spume/pull/35)).

## [0.3.0] - 2026-07-23

### Added

- Added the `CheckAddress` trait for parsing and validating addresses ([#31](https://github.com/aursen-labs/spume/pull/31)).

### Changed

- Accept `impl AsRef<str>` in address-taking APIs for better dev UX with frontends ([#27](https://github.com/aursen-labs/spume/pull/27)).
- Reworked address handling to use the `CheckAddress` trait ([#32](https://github.com/aursen-labs/spume/pull/32)).
- Dropped the `http` crate in favor of a raw `u16` status check ([#26](https://github.com/aursen-labs/spume/pull/26)).
- Dropped the redundant `Content-Length` size pre-check ([#25](https://github.com/aursen-labs/spume/pull/25)).
- Updated dependencies ([#30](https://github.com/aursen-labs/spume/pull/30)).

### Fixed

- Fixed a memory and connection leak in the WebSockets client ([#29](https://github.com/aursen-labs/spume/pull/29)).

## [0.2.0] - 2026-05-19

### Added

- Added custom HTTP header support on `WasmClient` requests ([#13](https://github.com/aursen-labs/spume/pull/13)).
- Added `Clone` and `Debug` implementations for `WasmClient` ([#9](https://github.com/aursen-labs/spume/pull/9)).
- Added `#[must_use]` annotations to client, provider, pubsub connect, and unsubscribe APIs ([#16](https://github.com/aursen-labs/spume/pull/16)).
- Added a configurable HTTP response size cap to protect wasm consumers from oversized RPC payloads ([#17](https://github.com/aursen-labs/spume/pull/17)).
- Added `WasmPubsubClient::is_connected` so consumers can inspect websocket connection state ([#18](https://github.com/aursen-labs/spume/pull/18)).
- Added integration coverage for `get_blocks` and `get_leader_schedule` ([#15](https://github.com/aursen-labs/spume/pull/15)), plus coverage for response size limits ([#17](https://github.com/aursen-labs/spume/pull/17)), custom headers ([#13](https://github.com/aursen-labs/spume/pull/13)), and `is_connected` ([#18](https://github.com/aursen-labs/spume/pull/18)).

### Changed

- Pinned the Rust toolchain in `rust-toolchain.toml` for more reproducible local and CI builds ([#12](https://github.com/aursen-labs/spume/pull/12)).

### Fixed

- Fixed inconsistent imports in the pubsub provider ([#10](https://github.com/aursen-labs/spume/pull/10)).
- Fixed live subscription streams so disconnects are surfaced to consumers ([#14](https://github.com/aursen-labs/spume/pull/14)).

## [0.1.0] - 2026-05-18

### Added

- Initial release.

[Unreleased]: https://github.com/aursen-labs/spume/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/aursen-labs/spume/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/aursen-labs/spume/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/aursen-labs/spume/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/aursen-labs/spume/releases/tag/v0.1.0
