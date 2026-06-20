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
| `splittr-crypto` | Identity/device keys, signing, E2E (#6/#14/#16) | ⏳ planned |
| `splittr-store` | Op-log + projection persistence (#1) | ⏳ planned |
| `splittr-sync` | iroh transport (#9/#20) | ⏳ planned |
| `splittr-app` | Use-cases / command + query API | ⏳ planned |
| `splittr-ffi` | flutter_rust_bridge surface (#23) | ⏳ planned |

## Develop

```bash
cargo test            # unit + property (proptest) tests
cargo test -- --nocapture
```

The headline guarantee (ADR-0001): the projection is a pure function of the op
**set** — property tests assert that any delivery order / duplication yields
identical state and balances.
