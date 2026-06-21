# Building Splittr

How to set up an environment to build, test, and package Splittr — a **Rust
engine (`splittr-core`)** with a **Flutter** presentation shell that loads the
engine over [`flutter_rust_bridge`](https://cjycode.com/flutter_rust_bridge/)
(FRB).

> **Where things stand:** the engine and the FFI are complete, and the **Linux
> desktop** build compiles, bundles, and runs on the engine end-to-end. The
> other platforms' native build hooks (Windows/macOS/Android/iOS/web) are the
> remaining tail of the FFI work — see [Other platforms](#other-platforms).

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| **Rust** | stable | `rustup` toolchain (edition 2021); ships `cargo`, `clippy`, `rustfmt`. |
| **Flutter** | stable channel | Bundles the matching Dart SDK (the app needs Dart `^3.12`). |
| **A platform toolchain** | per target | See below — e.g. GTK/clang for Linux. |

Verify the basics:

```bash
rustup toolchain install stable
rustup component add clippy rustfmt
flutter --version          # stable channel
flutter doctor             # resolve any reported toolchain gaps
```

### Linux (the wired desktop target)

The native engine is built with `cargo` and bundled next to the Flutter library;
you also need GTK (Flutter desktop) and libsecret (the `flutter_secure_storage`
keystore plugin):

```bash
sudo apt-get update
sudo apt-get install -y ninja-build libgtk-3-dev libsecret-1-dev clang cmake pkg-config
flutter config --enable-linux-desktop
```

## Repository layout

```
/                  Flutter app (lib/, android/ ios/ linux/ windows/ macos/ web/)
  lib/state/       engine-backed Riverpod facade (the one seam to Rust)
  lib/src/rust/    generated FRB bindings (do not edit by hand)
splittr-core/      Rust workspace (crates/splittr-*)
flutter_rust_bridge.yaml   FRB codegen config
```

## Build & test the engine (no Flutter needed)

The engine is a self-contained Cargo workspace. This is the fastest inner loop
and what the bulk of CI exercises:

```bash
cd splittr-core
cargo test --workspace                                  # unit + proptest (incl. convergence)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

## Regenerating the FRB bindings

`lib/src/rust/` is **generated** from the `splittr-ffi` crate's public API. The
checked-in bindings are kept in sync, so you only need this after changing the
FFI surface (`splittr-core/crates/splittr-ffi/src/api.rs` or `dto.rs`). Match the
codegen version to the `flutter_rust_bridge` package pinned in `pubspec.yaml`:

```bash
cargo install flutter_rust_bridge_codegen --version 2.12.0
flutter_rust_bridge_codegen generate        # reads flutter_rust_bridge.yaml
```

## Build & run the Flutter app (Linux)

```bash
flutter pub get
flutter run -d linux            # debug run; auto-builds + bundles the engine
```

How it fits together (see `linux/CMakeLists.txt`): the build adds a `splittr_ffi`
target that runs `cargo build -p splittr-ffi`, then installs the resulting
`libsplittr_ffi.so` into the bundle's `lib/` directory. The app is linked with an
`$ORIGIN/lib` rpath, so the FRB bindings `dlopen` the engine at runtime. The
Flutter build mode maps to the cargo profile (debug ↔ `--release`).

### Flutter tests

```bash
# The engine bridge test host-loads the cdylib, so build it first:
(cd splittr-core && cargo build -p splittr-ffi)
flutter test                    # widget tests + end-to-end engine bridge test
flutter analyze
```

## Packaging a Linux release

```bash
flutter build linux --release
```

The relocatable bundle lands in `build/linux/x64/release/bundle/` with the
engine bundled at `bundle/lib/libsplittr_ffi.so`. The whole `bundle/` directory
is self-contained — ship it as-is, or wrap it (e.g. AppImage, `.deb`, Flatpak).

A quick sanity check that the engine was bundled (the same assertion CI makes):

```bash
test -f build/linux/x64/release/bundle/lib/libsplittr_ffi.so && echo "engine bundled"
```

## Other platforms

Each target needs the same shape of hook — build the `splittr-ffi` crate for that
platform's ABI and bundle the resulting library so the FRB bindings can load it:

| Platform | Native toolchain | Engine build hook |
|---|---|---|
| **Linux** | clang + GTK + libsecret | ✅ wired (`linux/CMakeLists.txt`) |
| **Windows** | Visual Studio (MSVC) | ⏳ CMake hook pending |
| **macOS** | Xcode | ⏳ podspec / build-phase pending |
| **iOS** | Xcode + a device/simulator | ⏳ podspec pending |
| **Android** | Android SDK + NDK | ⏳ Gradle/cargo-ndk hook pending |
| **Web** | — | ⏳ WASM build pending |

Until those hooks land, build and run on **Linux desktop**. The engine itself is
platform-agnostic and already cross-compiles; only the per-platform bundling glue
is outstanding.

## Continuous integration

`.github/workflows/ci.yml` runs on every push and mirrors the commands above:

1. **Rust engine** — `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`.
2. **Flutter shell** — `flutter analyze`, build the FFI cdylib, `flutter test`
   (plus a guard that the removed v1 Dart JSON store stays gone).
3. **Linux desktop build** — `flutter build linux` and assert the engine got
   bundled.

Keep all three green before opening a PR.
