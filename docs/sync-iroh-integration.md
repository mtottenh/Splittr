# Sync & the iroh transport (#9 / #20)

How the `splittr-sync` crate is structured, what's done and verified, and the
remaining work to land iroh. Read alongside ADR-0002 (transport choice) and
ADR-0001 (why sync is a set-union of ops).

## The model

Sync is a **set-union of signed ops**; balances never cross the wire. Because the
fold is order-independent and ops are content-addressed (ADR-0001), two replicas
that hold the same op-set compute identical balances. So sync only has to make two
replicas' op-sets equal — then each folds independently.

```
local op ──▶ broadcast / reconcile ──▶ peer ingests ──▶ verify + fold ──▶ balances
```

## Layers (all in `splittr-sync`)

| Layer | What | Status |
|---|---|---|
| `message` | `SyncMessage` (`Have`/`Ops`), postcard wire encoding | ✅ done, tested |
| `reconcile` | `SyncStore` seam + `SyncSession` (pure protocol) + `missing_ops` | ✅ done, tested (incl. 200-case convergence proptest) |
| `transport` | `Transport` trait + bounded `sync()` driver | ✅ done, tested over an in-memory message duplex |
| `framed` | `FramedTransport` over any `AsyncRead`+`AsyncWrite` | ✅ done, tested over a tokio byte duplex |
| iroh wrapper | `Endpoint` setup → `FramedTransport` | ⬜ documented below; not yet wired |
| App/FFI | expose "sync with peer" to the shell | ⬜ next |

`SyncStore` is implemented for `Repository`, so `ingest` runs the full trust
boundary — `Op::verify` then the entitlement fold. Forged ops are rejected;
signed-but-unauthorized ops are stored but dropped by the fold (ADR-0006), so they
never affect balances.

## The reconciliation protocol

Fixed-shape, bounded, order-independent:

1. Initiator → `Have(ids)` — "I hold these op-ids."
2. Responder → `Have(ids)`, `Ops(delta)` — reciprocates its ids once and sends the
   ops the initiator lacks.
3. Initiator → `Ops(delta)` — sends the ops the responder lacks.

Each side sends and receives exactly two messages over an ordered, reliable
stream. Only the **ops** a peer is missing cross the wire (ids are cheap; a
range-based digest is a future bandwidth optimisation — see below). Re-running is
idempotent (dedup by content id).

## Wiring iroh (the remaining transport glue)

The device keypair (#16) **is** the iroh `NodeId` (both ed25519). Invites (#7)
already carry the inviter's identity; they should also carry a `NodeAddr` (NodeId
+ relay hint) so first contact works without a directory. Then a `Transport` is a
single QUIC bidirectional stream wrapped in `FramedTransport`:

```rust
const SPLITTR_ALPN: &[u8] = b"splittr/sync/1";

let endpoint = iroh::Endpoint::builder()
    .secret_key(device_secret_key)   // = our NodeId
    .alpns(vec![SPLITTR_ALPN.to_vec()])
    .bind()
    .await?;

// Initiator:
let conn = endpoint.connect(peer_addr, SPLITTR_ALPN).await?;
let (send, recv) = conn.open_bi().await?;
let mut t = FramedTransport::new(recv, send);
splittr_sync::sync(&mut repo, &mut t, Role::Initiator).await?;

// Responder (accept loop):
let conn = endpoint.accept().await?.await?;
let (send, recv) = conn.accept_bi().await?;
let mut t = FramedTransport::new(recv, send);
splittr_sync::sync(&mut repo, &mut t, Role::Responder).await?;
```

No extra protocol code is needed — `FramedTransport` already frames messages over
the stream, and that framing is tested over an in-memory byte duplex (the same
code path QUIC drives). Live fan-out to a whole group later uses `iroh-gossip` to
push new `Ops`; catch-up still uses the `sync()` exchange above.

## Why iroh isn't a dependency yet

- The version resolvable in this workspace is **iroh 0.95** (pre-1.0). It
  currently fails to build here via a transitive `ed25519-dalek`/`pkcs8` version
  conflict, unrelated to our code.
- P2P (QUIC hole-punching, relays) can't be exercised in CI/sandbox, so an iroh
  transport can't be runtime-verified here regardless.

So iroh is left out of the dependency graph until it builds cleanly on a pinned
toolchain and can be tested on real hosts — keeping the default build, MSRV (1.85)
and `cargo audit` light. The `Transport`/`FramedTransport` seam means dropping it
in (or choosing a different substrate) doesn't touch any sync correctness.

## Remaining work

- [ ] iroh dependency on a clean/pinned toolchain; the `IrohTransport` wrapper above.
- [ ] App/FFI surface: "sync with peer (NodeAddr)" + a background accept loop; push
      new local ops live (gossip) and reconcile on connect.
- [ ] Carry `NodeAddr` + relay hint in invites (#7).
- [ ] `iroh-blobs` for attachments (#4); zero-knowledge relay store-and-forward (#14).
- [ ] Range-based set reconciliation to shrink the id exchange for large logs.
- [ ] Loopback + two-host convergence tests over real iroh (off by default).
