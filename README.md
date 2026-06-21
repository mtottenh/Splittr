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
   ├─ splittr-domain   money (integer cents), ids, split/balance/debt math
   ├─ splittr-crdt     Op, HLC, conflict resolution, projection fold
   ├─ splittr-crypto   identity/device keys, signing, AEAD-at-rest, recovery
   ├─ splittr-store    durable op-log + projection (redb, encrypted)
   ├─ splittr-app      command/query use-cases (validate, authorize, emit ops)
   ├─ splittr-ffi      flutter_rust_bridge surface (Engine + view-model DTOs)
   └─ splittr-sync     iroh transport (QUIC, hole-punch, relay)      (planned)
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

Built engine-first; the Rust core and its FFI are in place, and the Flutter
shell runs on them end-to-end (Linux desktop today).

- ✅ **Engine** — `splittr-domain`, `splittr-crdt`, `splittr-crypto`,
  `splittr-store`, `splittr-app` and the `splittr-ffi` (flutter_rust_bridge)
  surface are implemented and tested: property-based convergence, durable
  encrypted persistence (redb), and 100+ Rust tests.
- ✅ **Flutter shell** — presentation only; the v1 Dart business logic has been
  removed. Every screen renders engine view-models through a Riverpod facade
  over the FFI (`lib/state/`).
- ✅ **Local security** — op-log encrypted at rest (XChaCha20-Poly1305), a
  biometric/PIN app lock, a cold root key with device-only daily opens, a BIP39
  recovery phrase, and device enrol/revoke with a pairing short-auth-string.
- 🚧 **Next** — `splittr-sync` (iroh peer-to-peer transport) and end-to-end
  encryption for synced ops, then the remaining per-platform build hooks (only
  Linux desktop bundles the engine today — see [`BUILDING.md`](BUILDING.md)).

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

**Flutter shell** (runs on the bundled Rust engine):

```bash
flutter pub get
flutter run -d linux        # Linux desktop bundles + loads the engine
flutter test                # widget tests + an end-to-end engine bridge test
flutter analyze             # strict static analysis
```

Full environment setup, the flutter_rust_bridge codegen step, and how the
native engine is bundled per platform live in [**BUILDING.md**](BUILDING.md).
