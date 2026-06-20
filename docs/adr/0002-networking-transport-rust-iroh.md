# ADR-0002 — Networking & transport: Rust interop via iroh

- **Status:** Proposed (recommendation; needs owner sign-off before adoption)
- **Date:** 2026-06-20
- **Deciders:** project owner + AI agent
- **Related:** ADR-0001 (the core this transports); Epic #18; issues #9, #20, #21, #6, #16, #14, #4.

## Context

ADR-0001 makes the core an op-log + deterministic fold, with **transport
deliberately left as a swappable concern** behind a `SyncTransport` interface.
The remaining question is how replicas actually exchange ops across devices and
users: device discovery, NAT traversal, an authenticated/encrypted channel,
efficient delta exchange, and large-blob transfer (receipts, #4).

Issues #20/#21 originally specced hand-rolling this in Dart (mDNS + sockets, then
WebRTC + a relay). Peer-to-peer connectivity — NAT hole-punching, relay
fallback, discovery — is one of the hardest things to build correctly. The
question raised: **should we interoperate with Rust and leverage
[iroh](https://www.iroh.computer/) instead of reinventing it?**

## Findings (verified June 2026)

- **iroh 1.0 shipped 2026-06-15** with stable wire-protocol guarantees and
  official **Swift, Kotlin, Python and Node.js** bindings. It is a modular Rust
  P2P stack built on **QUIC**, providing encryption, authentication, stream
  multiplexing, hole-punching (≈90% direct-connection success) with **relay
  fallback**, and multiple discovery mechanisms.
- **Identity model is a near-exact match for ours.** An iroh `NodeId` is a 32-byte
  **Ed25519 public key**; you "dial keys, not IPs." This is precisely our device
  identity model (#6/#16) — a device's keypair can *be* its iroh NodeId.
- **Browser/WASM works, relay-only.** Since iroh 0.33 it compiles to WASM;
  browsers can't send UDP from the sandbox, so they **cannot hole-punch** and are
  **always relayed** — but traffic stays **end-to-end encrypted, so the relay
  cannot read it**. So web is supported as a *degraded* transport, not excluded.
- **Layered protocols exist:** `iroh-gossip` (epidemic broadcast/pub-sub),
  `iroh-blobs` (BLAKE3 content-addressed, verified, resumable blob transfer), and
  `iroh-docs` (multi-writer signed key-value documents synced via **range-based
  set reconciliation** — active at 0.95). These map directly onto our needs.
- **Dart binding path is mature:** there is no official Dart binding, but
  **`flutter_rust_bridge`** (a Flutter Favorite) binds Rust to Dart across
  **Android, iOS, Windows, Linux, macOS and Web (WASM)**, with async + `Stream`
  support — exactly the surface we need to expose a `SyncTransport`.

## Decision (proposed)

**Adopt iroh as the transport substrate, exposed to Flutter through a small Rust
crate (`splittr-sync`) via `flutter_rust_bridge`, kept strictly behind the
`SyncTransport` interface. Keep all CRDT/op-log semantics (ADR-0001) in pure
Dart. Borrow range-based set reconciliation; use `iroh-blobs` for attachments.
Do not couple our conflict semantics to `iroh-docs`.**

Concretely:

1. **Pure-Dart core stays pure (hard rule).** The op model, HLC, fold, conflict
   resolution and convergence tests (#19) have **zero** dependency on Rust/iroh.
   They are tested with an in-memory transport. This is what makes the iroh
   decision low-risk and reversible — and is why it can be deferred to Phase 1
   without holding up the data-model foundation (Phase 0).

2. **`splittr-sync` Rust crate** wraps iroh connectivity + gossip and exposes a
   minimal FFI surface: `start(nodeKey)`, `joinGroup(scope, peers)`,
   `broadcastOp(bytes)`, `onOp(stream)`, `reconcile(peer, haveSummary)`,
   `fetchBlob(hash)` / `putBlob(bytes)`. Dart implements `IrohSyncTransport`
   against it.

3. **Identity unification.** The device keypair (#16) is the iroh NodeId. Invites
   (#7) carry the NodeId + a relay hint so first contact works without a
   directory. QUIC's mTLS authenticates the hop; **content E2E encryption (#14)
   is still required** so relays remain zero-knowledge (QUIC only protects the
   link to the relay, not from it).

4. **Delta sync = range-based set reconciliation** over our content-addressed op
   ids (ADR-0001). Spike two options before committing: **(a)** store ops as
   entries in an `iroh-docs` namespace and let it replicate/reconcile;
   **(b)** use `iroh-gossip` for live fan-out + our own range reconciliation for
   catch-up. Lean (b) for maximum control over semantics and testability, but
   evaluate (a) for the engineering it saves.

5. **Attachments via `iroh-blobs`** (#4): content-addressed, verified, resumable;
   solves "don't naively relay large images."

6. **Web** uses the same crate compiled to WASM, relay-only, E2E-preserved — or,
   if WASM+iroh proves immature in practice, a thin WebSocket-to-relay fallback.
   Note: web was a bonus target, never a hard requirement (the brief was native
   iOS/Android/Windows/Linux), so a degraded web transport is acceptable.

## Consequences

**Positive**
- We do **not** reinvent NAT traversal / relays / discovery — the single hardest,
  most failure-prone part of the foundation is delegated to a 1.0, production-used
  library whose identity model already matches ours.
- QUIC gives authenticated, encrypted, multiplexed streams out of the box.
- `iroh-blobs`/`iroh-gossip` cover attachment transfer and live dissemination.
- iroh becomes one implementation of `SyncTransport`; an in-memory transport is
  used for tests and a pure-Dart relay fallback remains possible. Low lock-in.

**Negative / costs**
- **Polyglot build.** Adds a Rust toolchain, cross-compilation for every target
  (iOS arm64, Android NDK, desktop triples, WASM), `flutter_rust_bridge` codegen,
  larger binaries, and more CI. Real, but well-trodden with FRB.
- **Two languages to maintain** — mitigated by keeping the Rust surface tiny and
  semantics-free (pure transport).
- **Web is relay-only** (no P2P hole-punching in browser).
- **`iroh-docs` is pre-1.0 (0.95)** and evolving — hence we do not hard-depend on
  it for semantics; iroh *connectivity* is the 1.0, stable part we rely on.

**Risk controls**
- The `SyncTransport` boundary + pure-Dart core mean we can ship the entire
  foundation (Phase 0) and even a first transport with an in-memory/relay
  implementation, then drop iroh in (or back out) without touching correctness.
- Time-box a spike (below) before committing build complexity.

## Impact on the roadmap

- **No change to Phase 0 (foundation/data model).** It is pure Dart and
  transport-independent. This is the key point: we can build the part that is
  "imperative to get correct" now, and the Rust/iroh choice only lands in
  Phase 1.
- **#20 and #21 are largely superseded** by "integrate iroh via `splittr-sync`."
  On acceptance, recommend: collapse #20/#21 into **(i)** a foundation-adjacent
  issue *"Rust FFI scaffold: `splittr-sync` crate + flutter_rust_bridge + CI
  cross-compilation"* and **(ii)** *"iroh transport: discovery, gossip,
  reconciliation, blobs behind `SyncTransport`"*, with a web-relay fallback
  sub-task. Keep #14 (content E2E) unchanged and still required.

## Validation spike (before acceptance)

1. Stand up `splittr-sync` exposing `start`/`broadcastOp`/`onOp` over iroh-gossip;
   bind via FRB; round-trip an op between two desktop instances.
2. Confirm Android + iOS builds of the FFI crate in CI.
3. Prototype range reconciliation of a 10k-op log (custom vs `iroh-docs`).
4. Confirm a relayed connection between a native node and a WASM/web node, with a
   test asserting the relay sees only ciphertext.

## Alternatives considered

- **Hand-rolled Dart transport (original #20/#21):** maximal portability and no
  Rust, but we would reimplement hole-punching/relays/discovery worse and slower.
  Rejected as the primary path; retained as the in-memory + relay fallback.
- **libp2p (Rust or other):** capable but heavier and less identity-aligned than
  iroh for our use; `libp2p-iroh` exists if we later want libp2p semantics over
  iroh transport.
- **Depend on `iroh-docs` as our CRDT:** tempting (it would host the op-set), but
  couples core semantics to a pre-1.0 crate. Rejected for the *semantics*;
  acceptable to evaluate as a *replication mechanism* in the spike.

## Sources

- iroh 1.0 / overview: https://www.iroh.computer/blog/v1 ,
  https://github.com/n0-computer/iroh , https://docs.iroh.computer/about/faq
- WASM/browser support: https://docs.iroh.computer/deployment/wasm-browser-support ,
  https://www.iroh.computer/blog/iroh-and-the-web
- Set reconciliation / docs: https://docs.rs/crate/iroh-docs/latest ,
  https://docs.iroh.computer/protocols/documents
- Dart binding: https://pub.dev/packages/flutter_rust_bridge
