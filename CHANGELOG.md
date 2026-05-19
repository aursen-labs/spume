# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/aursen-labs/spume/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/aursen-labs/spume/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/aursen-labs/spume/releases/tag/v0.1.0
