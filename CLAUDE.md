# CLAUDE.md

Guidance for AI agents working in this repo. Keep it short; link, don't duplicate.

## What this is
Cross-platform Splitwise clone: a **Rust engine (`splittr-core`)** with a
**Flutter shell**. Local-first, event-sourced, CRDT-convergent.
Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/adr/`](docs/adr/)
before changing the core; the GitHub **Epic** is the roadmap.

## Non-negotiable invariants
- **Source of truth = the signed op-log; state = a deterministic fold** over it.
  Never mutate state in place; add an op and let the `Materializer` fold it.
- **Conflict rules live in one place** (`splittr-crdt`): whole-version LWW edits,
  delete-wins tombstones, LWW scalars/membership, alias union-find. Changing them
  means updating ADR-0001 and the property tests.
- **Money is integer cents.** Splits must reconcile exactly.
- **Determinism**: no map-iteration-order / wall-clock / locale dependence in the
  fold. The convergence property test is the guardrail — keep it green.
- **Flutter is presentation only.** Business logic belongs in Rust.
- The v1 Dart logic in `lib/` is being retired; don't add new business logic there.

## Where things live
- `splittr-core/crates/splittr-{domain,crdt,store,...}` — the engine.
- `lib/` — Flutter app (v1, being migrated to call the FFI).
- `docs/adr/` — decisions; `docs/ARCHITECTURE.md` — the current map.

## Dev commands
```bash
# Rust core
cd splittr-core && cargo test && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
# Flutter shell
flutter test && flutter analyze
```

## Conventions
- **Foundations first**, and **property-test the core** (proptest) — not just examples.
- **DRY + small modules**: one concern per file; no monolithic files; no duplicate
  implementations (e.g. one `lww`, one `Materializer`).
- Keep `docs/ARCHITECTURE.md`, the ADRs, and the issue tracker in sync with code.
- Commit messages reference issues (e.g. `#19`); keep ADR status accurate.
