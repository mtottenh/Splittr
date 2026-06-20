# Splittr

A cross-platform **expense-sharing app** (a Splitwise clone) for **iOS, Android,
Windows, Linux, macOS and the web** — built as a **Rust engine with a Flutter
presentation shell**.

> ### ℹ️ About this project — an experiment in autonomous AI coding
>
> Splittr is an experiment to see just how far autonomous AI coding has come.
> The application — its architecture, implementation, tests, design records
> (`docs/adr/`) and the GitHub issue roadmap — is being built by an autonomous AI
> coding agent (Claude), with a human directing at the level of **goals and
> architectural decisions** rather than line-by-line code. Treat it as a living
> demonstration of that collaboration, not a finished product.

## Architecture

The core logic lives in **Rust** (`splittr-core`); **Flutter** is a thin
presentation shell over it via [`flutter_rust_bridge`](https://pub.dev/packages/flutter_rust_bridge).
The design is **local-first and event-sourced**: the source of truth is an
append-only log of signed operations, and all state (groups, expenses, balances)
is a deterministic, **CRDT-convergent** fold over that log — so devices and users
sync peer-to-peer without a central authority.

```
Flutter shell (Dart)         presentation only — screens, theme, navigation
   │  commands / view-model streams (FFI: flutter_rust_bridge)
Rust splittr-core
   ├─ splittr-domain   money (integer cents), ids, split/balance math
   ├─ splittr-crdt     Op, HLC, conflict resolution, projection fold
   ├─ splittr-crypto   identity/device keys, signing, E2E            (planned)
   ├─ splittr-store    durable op-log + projection (redb) + Repository
   ├─ splittr-sync     iroh transport (QUIC, hole-punch, relay)      (planned)
   └─ splittr-app/ffi  use-cases + FFI surface                       (planned)
```

Why this shape (full rationale in the ADRs):

- **Flutter** is the only mainstream toolkit that targets all four required
  platforms (iOS/Android/Windows/Linux) natively from one codebase — React
  Native and .NET MAUI both have desktop/Linux gaps. *(ADR context; see below.)*
- **Rust core** because the correctness-critical part (CRDT convergence, crypto,
  canonical encoding) is best served by Rust's type system and property-testing
  ecosystem — and we already pay for Rust+FFI to use **iroh** for P2P transport.
  *(ADR-0003.)*
- **Money is integer cents** everywhere; splits reconcile to the penny.
- **Conflict resolution is deterministic** (whole-version LWW for edits,
  delete-wins tombstones, alias union-find for identity merges) so every replica
  computes identical balances. *(ADR-0001.)*

Read next:
[**docs/ARCHITECTURE.md**](docs/ARCHITECTURE.md) (the system map) ·
[**docs/adr/**](docs/adr/) (decision records) · the GitHub **Epic** (roadmap).

## Status

This is a migration in progress, developed foundations-first:

- ✅ **Rust core foundation** — `splittr-domain`, `splittr-crdt`, `splittr-store`
  implemented and tested (property-based convergence + durable persistence).
- 🚧 **Next** — `splittr-app` (command/query use-cases), the FFI scaffold, then
  rebinding the UI and retiring the v1 Dart logic.
- 📦 **v1 Flutter app** — a complete, runnable Dart implementation still lives in
  `lib/`; it is being replaced layer-by-layer by the Rust core (its split/balance
  logic currently doubles as a differential-test oracle for the Rust port).

## Features

Groups and one-to-one friend balances · expenses split **equally / by exact
amounts / by percentage / by shares** · categories · multi-currency · per-member
balances and an overall "you owe / are owed" · **settle up** with debt
simplification · activity feed · offline-first. (Advanced features — contacts,
invites, multi-device sync, receipts/OCR, payments, open-banking import — are
tracked in the issue roadmap.)

## Repository layout

```
/                  Flutter app (lib/, android/ ios/ linux/ windows/ macos/ web/)
splittr-core/      Rust workspace (crates/splittr-*)
docs/
  ARCHITECTURE.md  system map
  adr/             architecture decision records
CLAUDE.md          guidance for AI agents working in this repo
```

## Develop

**Rust core** (the engine — no Flutter needed):

```bash
cd splittr-core
cargo test            # unit + property (proptest) tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

**Flutter shell** (the current v1 app):

```bash
flutter pub get
flutter run                 # auto-selects a device; or -d linux|windows|chrome|<id>
flutter test                # widget/unit tests
flutter analyze             # strict static analysis
```

Desktop builds need that platform's native toolchain (clang + GTK for Linux,
Visual Studio for Windows, Xcode for Apple). Building the Rust core into the
Flutter app (cross-compilation + codegen) arrives with the FFI scaffold.
