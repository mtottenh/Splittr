# Architecture Decision Records

Each ADR captures one significant architectural decision: its context, the
decision, and the consequences. They are the durable rationale behind the
codebase and the roadmap (tracked in GitHub issues, see Epic #18).

For the *current coherent system picture* (how these decisions fit together),
see **[../ARCHITECTURE.md](../ARCHITECTURE.md)**.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-event-sourced-crdt-core.md) | Event-sourced, CRDT-convergent core data model | Accepted |
| [0002](0002-networking-transport-rust-iroh.md) | Networking & transport: Rust interop via iroh | Accepted (transport); language split superseded by 0003 |
| [0003](0003-core-language-rust-vs-dart.md) | Core implementation language: Rust-core vs Dart-core | Accepted |

Status values: **Proposed** (recommended, awaiting sign-off) · **Accepted**
(decided) · **Superseded** (replaced by a later ADR).
