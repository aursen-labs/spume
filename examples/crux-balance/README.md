# crux-balance

A [Crux](https://redbadger.github.io/crux/) core that reads a Solana account
balance, with iOS/macOS and Android shells. Laid out like the upstream
[`counter-http`](https://github.com/redbadger/crux/tree/master/examples/counter-http)
example, so anything you learn there applies here.

| iOS (SwiftUI) | Android (Jetpack Compose) |
| --- | --- |
| <img src="screenshots/ios.png" alt="iOS balance example showing 0.000000001 SOL" width="280"> | <img src="screenshots/android.png" alt="Android balance example showing 0.000000001 SOL" width="280"> |

```
shared/     the Rust core: app, FFI surface, type codegen
apple/      SwiftUI shell (iOS + macOS), Xcode project generated from project.yml
Android/    Jetpack Compose shell
```

## The point

A Crux core asks its shell to perform I/O. Enable `spume`'s optional `crux`
feature with `default-features = false` to use the typed client without its
browser transport:

```rust
let client = spume::CruxClient::new(RPC_URL, ctx.clone());
let balance = client.get_balance(&address, None).await?;
```

This example validates the address in `update` with `spume::rpc::get_balance`,
then passes the resulting typed call to `CruxClient::send`. Invalid addresses
never reach the shell. The client handles JSON content types and preserves
JSON-RPC errors even when the server returns a non-2xx HTTP status.

The existing shells only handle `crux_http` effects; they carry bytes without
knowing about Solana. See the root README for the `CruxPubsubClient` and its
additional WebSocket shell protocol.

## Run it

### Core

Needs nothing but Rust. The tests answer the HTTP effect themselves, so the
whole round trip runs with no network and no shell:

```bash
cargo test -p shared
```

### iOS / macOS

Needs [`xcodegen`](https://github.com/yonaskolb/XcodeGen), `boltffi`
(`cargo install boltffi_cli`) and Xcode.

```bash
just apple/build     # typegen + boltffi pack apple + xcodegen + xcodebuild
just apple/open      # …or open the generated project in Xcode
```

`apple/generated/` holds two Swift packages, both produced by the build:
`Shared` (the core as a static library plus its FFI bindings, from
`boltffi pack apple`) and `App` (`Event`, `ViewModel`, `Effect` as Swift types,
from `shared/src/bin/codegen.rs`). Neither is checked in, and neither is the
`.xcodeproj` — `apple/project.yml` regenerates it.

### Android

```bash
cd Android
just build           # boltffi pack android + typegen + assembleDebug
just install         # …onto a running device or emulator
```

Prerequisites, and the two that cost time:

- `cargo install boltffi_cli` — note the `_cli`. `cargo install boltffi` fails
  with "there is nothing to install"; that crate is the library the core links
  against, not the packaging tool.
- **A JDK 21.** Android Studio's bundled JBR is Java 25, which Gradle rejects
  with `Unsupported class file major version 69`;
  `gradle/gradle-daemon-jvm.properties` pins the daemon to 21 for that reason.
  `brew install openjdk@21`, then `JAVA_HOME=/opt/homebrew/opt/openjdk@21` —
  which is what the Justfile defaults to.
- The Android SDK and an NDK under `$ANDROID_HOME/ndk` (SDK Manager → SDK Tools
  → NDK), or `ANDROID_NDK_HOME` pointing at one. Built and verified here against
  r27d. Without it `boltffi pack android` stops at `android ndk not found`.
- `Android/local.properties` with `sdk.dir=…`, or `ANDROID_HOME` in the
  environment.

`Android/generated/` holds the Kotlin bindings, the app types, and a `.so` per
ABI. The debug APK is ~200 MB because it carries four unstripped ABIs — pass
`--release` to `boltffi pack android` for a realistic one.

## What is not here

The upstream example also ships web shells (Leptos, Next.js) and a server-sent
events capability. Neither is needed to show the pattern, and for the browser
you would use `spume` the normal way — with its own transport — rather than
through a core.
