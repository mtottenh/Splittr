# ADR-0003 — Core implementation language: Rust-core engine vs Dart-core

- **Status:** Proposed (reconsiders the language split in ADR-0002)
- **Date:** 2026-06-20
- **Deciders:** project owner + AI agent
- **Related:** ADR-0001 (the data model this implements), ADR-0002 (transport via iroh), Epic #18.
- **New input:** the project owner is more fluent in **Rust** than Dart/Flutter. Stated priorities: **data-model correctness** and **clean, composable interface design**.

## Context

ADR-0001 fixes *what* the core is (a signed op-log + deterministic fold + per-entity CRDT rules). ADR-0002 proposed implementing that core in **pure Dart**, with Rust used only for the **iroh transport** behind a narrow `SyncTransport` FFI. This ADR asks the deeper question: **should the entire core — data model, event log, CRDT, crypto, persistence, sync — be written in Rust**, with Flutter reduced to a presentation shell?

Two approaches:

- **A — Dart-core + Rust-transport** (ADR-0002 as written). FFI line sits at the *transport* (opaque op-bytes).
- **B — Rust-core + Flutter-shell** ("headless engine"). FFI line moves *up* to an application command/query/subscription API; Flutter is a driving adapter.

## Approach B in concrete terms

A Cargo **workspace** of small, composable crates with trait-defined seams — the kind of decomposition Rust's type system rewards:

```
splittr-core/                # pure, deterministic, no I/O
  splittr-domain   # models, money, split/balance/debt math (the v1 Dart logic, ported)
  splittr-crdt     # Op, Hlc, conflict resolution, projection/fold  (ADR-0001)
  splittr-crypto   # identity keys, signing, E2E (ed25519/x25519/blake3)
splittr-store/               # trait Store; impls: sqlite (rusqlite) / redb
splittr-sync/                # trait Transport; impl: iroh (ADR-0002)
splittr-app/                 # use-cases: dispatch(Command) / watch(Query)->Stream
splittr-ffi/                 # flutter_rust_bridge surface over splittr-app
```

- **FFI port = commands in, view-models / state-streams out.** Flutter sends
  `Command::AddExpense{..}` and subscribes to typed view-models; it owns nothing
  but presentation. This is a hexagonal architecture with the FFI as the port.
- **iroh is an in-process dependency**, not a sub-FFI — the whole engine *and*
  transport are one Rust program.
- The same `splittr-core` powers a **relay/server and a CLI** unchanged.

## Contrast by dimension

### Data model (the priority)
- **B (Rust) is stronger.** Sum types with payloads + exhaustive `match` make the
  op/event vocabulary and the conflict rules of ADR-0001 a natural fit; adding an
  op variant fails to compile until handled everywhere. Zero-cost **newtypes**
  (`Cents(i64)`, `OpId([u8;32])`, `Hlc`) enforce invariants the v1 Dart code only
  conventions. Ownership makes "ops are immutable" a compiler guarantee, not a
  `@immutable` lint. Deterministic folds are easier to keep honest (`BTreeMap`,
  explicit ordering) — and determinism *is* convergence.
- **A (Dart) is adequate.** Dart 3 sealed classes, pattern matching and records
  model ops fine; sound null safety helps. But invariants stay conventions, and
  immutability is discipline.

### Interfaces & composability (the other priority)
- **B composes better at the macro level:** one engine behind one well-defined
  port; crates with `Store`/`Transport`/`Clock` traits are independently testable
  and swappable. The conceptual layering is pristine (engine vs presentation).
- **A has a smaller FFI surface** (just bytes), but the **sync responsibility is
  split across two languages** — range reconciliation needs the op-id index;
  putting it in Rust-transport muddies the seam, putting it in Dart reduces iroh
  to a dumb pipe. B has no such seam.
- **Cost of B:** the FFI surface is *richer* (commands + view-models must be
  modeled in Rust and code-generated to Dart), and the UI's reactivity must be
  fed by a **custom state-stream API** — whereas A gets reactive queries free
  from Drift.

### Ecosystem
- **Core engine → Rust wins decisively.** CRDT (`automerge`, `yrs`, `crdts`,
  `iroh-docs`), P2P (`iroh`, `quinn`, `libp2p`), crypto (RustCrypto:
  `ed25519-dalek`, `x25519-dalek`, `blake3`), and **`serde` + `postcard`/`bincode`
  for canonical, deterministic binary encoding** — which is exactly what
  content-addressed op-ids and signatures need. Dart's equivalents are thinner
  (`cryptography` is good but smaller; CRDT support is sparse; serialization is
  JSON-first, so canonical encoding is hand-rolled).
- **UI → Flutter/Dart wins**, unchanged in both approaches.

### Testing (owner emphasized rigor)
- **B wins.** `proptest`/`quickcheck` make ADR-0001's headline **convergence
  property test** (random op orderings/partitions → identical state) idiomatic;
  the *entire* core (incl. persistence + crypto + sync logic) is testable with
  `cargo test`, fast and deterministic, no Flutter harness. A would hand-roll
  property generators in a weaker Dart testing ecosystem.

### Dependency management & suitability
- **B:** one `Cargo.toml`/workspace with Cargo.lock — best-in-class, reproducible,
  feature-flagged; the owner's home turf. Dart side shrinks to UI packages + FRB.
- **A:** two ecosystems (pub.dev for crypto/persistence + a hand-written CRDT,
  *plus* cargo for iroh). More owned CRDT code in the thinner ecosystem.

### Build & platforms
- **Near-parity on cost, because A already ships Rust + FFI for iroh.** The
  marginal build cost of moving the core into Rust is therefore small — this is
  the pivotal point (below).
- **Web is the one place A is easier:** B must run the whole engine in WASM,
  where **Rust persistence in the browser (OPFS/IndexedDB) is the genuine hard
  part**; A already has web persistence via `drift` + `sqlite3.wasm`. Web is a
  bonus target (brief was native iOS/Android/Windows/Linux), so B can ship web as
  relay-only with limited/OPFS persistence.
- **Inner loop:** A keeps fast Dart hot-reload for UI without touching Rust;
  B rebuilds Rust on core changes (UI hot-reload still works).

### Reusability
- **B:** `splittr-core` is a portable library — the **relay/server (#21) and
  client share one crate for op validation and types**, eliminating a class of
  client/server divergence bugs. A CLI/headless daemon comes nearly free.
- **A:** core is Dart-only; a Rust relay would reimplement validation.

### Performance
- Irrelevant at this scale; slight edge to B for large folds/crypto. Not a
  deciding factor.

## Decision matrix

| Dimension | A: Dart-core + Rust-transport | B: Rust-core + Flutter-shell |
|---|---|---|
| Data-model expressiveness | Good (Dart 3 sealed/records) | **Best (ADTs, newtypes, ownership)** |
| Convergence property testing | Weak (hand-rolled) | **Strong (proptest/quickcheck)** |
| CRDT / crypto / serde ecosystem | Thin | **Deep** |
| Canonical binary encoding | Manual | **Idiomatic (serde+postcard)** |
| P2P (iroh) integration | Rust behind a sub-FFI seam | **In-process, no seam** |
| FFI surface | **Minimal (bytes)** | Richer (commands/view-models) |
| Reactive UI integration | **Free (Drift streams)** | Custom bridge needed |
| UI iteration speed | **Fast (pure Dart)** | Slower for core-touching changes |
| Web/WASM persistence | **Easier (drift wasm)** | Harder (OPFS from Rust) |
| Dependency management | Two ecosystems | **One core (cargo)** |
| Build complexity | Rust+FFI already required | ~Same (already paid) |
| Core reuse (relay/CLI) | Dart-only | **Shared Rust crate** |
| Team fit (owner Rust-first) | Core partly in weaker lang | **Core in strongest lang** |

## The pivotal insight

ADR-0002 **already commits us to Rust + FFI** (for iroh). Given that sunk cost,
the *marginal* cost of putting the correctness-critical core in Rust is modest,
while the benefits land squarely on the stated priorities (data-model
correctness, composable interfaces, rigorous convergence testing) **and** on the
owner's Rust fluency. We pay for the FFI either way; B gets far more value from it.

## Decision (proposed)

**Adopt Approach B: a Rust `splittr-core` engine + Flutter presentation shell.**
ADR-0001's semantics are unchanged — they are simply *implemented in Rust*.
ADR-0002's iroh choice stands but is absorbed **in-process** rather than behind a
sub-FFI. Flutter remains the UI for all platforms via `flutter_rust_bridge`.

### When A would still be the better call
- If the team were Flutter-first rather than Rust-first.
- If **web** were a first-class target needing rich offline persistence.
- If fastest UI iteration and a minimal FFI surface outweighed core rigor.
- If we wanted to avoid designing a reactive command/query bridge.

Given the actual inputs, these do not dominate.

## Consequences

**Positive:** the hard, correctness-critical core lives in the owner's strongest
language, with the best CRDT/crypto/serde/testing ecosystem; one composable
engine behind a clean port; the core is reusable for the relay/CLI; convergence
is property-tested in `cargo test`.

**Negative / risks:** a richer FFI surface and a **custom reactive bridge**
(Rust state-streams → Riverpod) — the main ergonomic tax; **web WASM persistence**
is a real wart (mitigate: relay-only/limited web, or OPFS-sqlite); slower
inner loop for core-touching UI work; cross-language debugging; the small v1 Dart
domain layer gets **re-ported to Rust** (it is ~a few hundred lines and gains
from the move).

## Impact on prior ADRs / roadmap

- **ADR-0001:** unchanged (semantics); now realized as `splittr-crdt` in Rust.
- **ADR-0002:** transport choice (iroh) stands; "core in pure Dart" is superseded
  by this ADR if accepted.
- **Roadmap:** Phase 0 issues (#1, #19, #15, #6, #16, #2, #22) become
  Rust-crate work + a thin FFI; add a foundational issue *"`splittr-core`
  workspace + flutter_rust_bridge + CI cross-compilation/codegen"*. The UI built
  in v1 is preserved and rebound to the FFI command/query API.

## Validation spike (before acceptance)

1. `splittr-core` skeleton: `Op`/`Hlc`/projection + one command + a `proptest`
   convergence test (proves the testing story).
2. FRB round-trip: Flutter dispatches `AddExpense`, watches a balances stream
   rendered with Riverpod (proves the reactive bridge).
3. CI builds the FFI crate for Android + iOS + one desktop target.
4. Spike WASM persistence (OPFS) to size the web wart.

## Open questions
- Reactive bridge granularity: stream whole view-models vs diffs?
- Persistence engine: `rusqlite` (SQL, familiar, WASM-awkward) vs `redb`
  (pure-Rust, simple, weak WASM)?
- Reconciliation: reuse `iroh-docs` vs custom range-reconciliation over the
  op-log (revisit ADR-0002's spike under a Rust core).
