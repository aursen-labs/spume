# crux-balance

A [Crux](https://redbadger.github.io/crux/) core that reads a Solana account
balance, with iOS/macOS and Android shells. Laid out like the upstream
[`counter-http`](https://github.com/redbadger/crux/tree/master/examples/counter-http)
example, so anything you learn there applies here.

```
shared/     the Rust core: app, FFI surface, type codegen
apple/      SwiftUI shell (iOS + macOS), Xcode project generated from project.yml
Android/    Jetpack Compose shell
```

## The point

A Crux core is side-effect free and compiles for iOS and Android, so it cannot
use `spume`'s wasm transport. It can still use the typed method list:

```rust
let call = spume::rpc::get_balance(&address, None)?;   // Call<Response<u64>>

let response = Http::post(RPC_URL)
    .body(call.body(1))          // params assembled by spume
    .content_type(crux_http::mime::APPLICATION_JSON)
    .build()
    .into_future(ctx)
    .await;

let balance = call.parse(body, status)?;               // result type from the same call
```

`spume` is a dependency with `default-features = false` — no `gloo-net`, no
`web-sys`, nothing that would fail to build for `aarch64-apple-ios`. The shells
never learn that JSON-RPC is involved; they carry bytes.

Three things worth stealing from `shared/src/app.rs`:

- **`fetch` is generic over the result type** (`Call<R> → R`), because the call
  already knows it. Every other `spume::rpc::*` builder works through the same
  helper unchanged.
- **A 4xx/5xx is handed back to `call.parse`.** `crux_http` turns an error status
  into `HttpError::Http { code, body }`; a Solana RPC reports `Node is behind` as
  a JSON-RPC error on a 500, so parsing the body anyway is what surfaces the real
  message instead of a bare "HTTP 500".
- **`check_address` works without the transport**, so a malformed address fails
  in `update` and no effect ever reaches the shell.

## Run it

### Core

Needs nothing but Rust. The tests answer the HTTP effect themselves, so the
whole round trip runs with no network and no shell:

```bash
cargo test -p shared
```

### iOS / macOS

Needs [`xcodegen`](https://github.com/yonaskolb/XcodeGen), `boltffi`
(`cargo install boltffi`) and Xcode.

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
