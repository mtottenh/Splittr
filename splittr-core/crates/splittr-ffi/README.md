# splittr-ffi

The [flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/) API surface
over `splittr-app` — the contract between the Rust engine and the Flutter shell
(#23). See [`../../../docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md) §5.

- **`Engine`** — an opaque handle (the local `App` behind a `Mutex`) with
  string/int methods Dart can call: profiles, people, groups, members, expenses
  (with a `SplitPlanDto`), settlements, plus `groups()` / `group_detail()`.
- **`dto`** — flutter_rust_bridge-friendly types (only `String`, ints, `Option`,
  `Vec`, plain structs/enums).

The crate is plain, FRB-friendly Rust and is unit-tested directly
(`cargo test -p splittr-ffi`).

## Generating the Dart bindings

Codegen + native bundling is a build step (run in CI / locally), not part of
`cargo test`:

```bash
cargo install flutter_rust_bridge_codegen   # once
flutter_rust_bridge_codegen generate        # uses ../../flutter_rust_bridge.yaml
```

This writes Dart bindings to `lib/src/rust/`. The native library is compiled
into the Flutter build via the flutter_rust_bridge / cargokit integration and
cross-compiled per target (Android NDK, iOS, desktop); the web target uses the
WASM build. The Flutter shell then calls `Engine.open(...)` and the command/query
methods (#24).
