# splittr-core

The Rust engine for Splittr (ADR-0003): event-sourced, CRDT-convergent, with the
Flutter app as a presentation shell over an FFI. See
[`../docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) and
[`../docs/adr/`](../docs/adr/).

## Crates

| Crate | Responsibility | Status |
|---|---|---|
| `splittr-domain` | Money (`Cents`), id newtypes, split/balance math | ✅ initial |
| `splittr-crdt` | `Op`, HLC, content-addressed ids, conflict resolution, projection fold | ✅ initial |
| `splittr-crypto` | Ed25519 identity keys + op signatures (#6); E2E (#14) later | ✅ initial (sign/verify) |
| `splittr-store` | Op-log + projection persistence; `Repository` (#1) | ✅ initial (in-memory + redb) |
| `splittr-sync` | iroh transport (#9/#20) | ⏳ planned |
| `splittr-app` | Command/query use-cases over `Repository` (#25) | ✅ initial |
| `splittr-ffi` | flutter_rust_bridge surface (#23) | ⏳ planned |

## Develop

```bash
cargo test            # unit + property (proptest) tests
cargo test -- --nocapture
```

The headline guarantee (ADR-0001): the projection is a pure function of the op
**set** — property tests assert that any delivery order / duplication yields
identical state and balances.
