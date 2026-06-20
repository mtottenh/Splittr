# ADR-0004 — Identity & key hierarchy

- **Status:** Accepted (2026-06-20)
- **Date:** 2026-06-20
- **Deciders:** project owner + AI agent
- **Related:** ADR-0001 (signed op-log), ADR-0003 (Rust core); issues #6 (identity primitives), #16 (multi-device), #14 (E2E), #22 (at-rest/app lock).

## Context

The engine signs every op (ADR-0001) and will, later, end-to-end encrypt synced
ops (#14). That requires a real, verifiable **identity** and a clear **key
hierarchy**: which keys exist, what each is for, how they are derived, and where
they are stored. Identity must be **decentralized by default** — a locally
generated keypair, *not* a mandatory server account — and the app must be fully
functional with no account.

## Decision

### Keys

| Key | Type | Purpose | Derivation / storage |
|---|---|---|---|
| **Identity signing key** | Ed25519 | Signs ops; its public key *is* the user id (`id:<hex>`). | From a 32-byte seed; seed in the OS keystore (`flutter_secure_storage`). |
| **Agreement key** | X25519 | Key agreement → per-recipient content keys for E2E (#14). | Derived from the **same** 32-byte seed (`AgreementKey::from_seed`). |
| **At-rest key** | XChaCha20-Poly1305 (256-bit) | Encrypts the local op-log on disk (#22). | Separate random 32-byte key in the OS keystore; never stored next to the DB. |
| Device keys / certs | (later) | Per-device enrolment, revocation. | **#16** — out of scope here. |
| Group content keys | (later) | Per-group symmetric key, wrapped per recipient via X25519. | **#14**. |

### Identity facts converge through the op-log

- `UpsertProfile { user, name }` — display profile (LWW).
- `SetAgreementKey { user, key }` — publishes the X25519 public key (LWW) so any
  peer can derive a shared secret to encrypt to that user. Emitted automatically
  when the local profile is set; content-addressed, so re-emitting is a no-op.

The local user's keys are exposed by the app (`my_agreement_public`) and FFI for
the eventual transport/encryption layers.

### Managed account = optional, later

A **managed account** (email/OAuth bound to the identity public key, for relay
discovery and encrypted backup) is an *optional* layer that depends on the relay
(#20). The decentralized keypair is the source of truth; an account only ever
*points at* it. Tracked separately so the core stays account-free.

## Consequences

- Both signing and agreement keys come from one seed → one secret to back up
  (#16 device enrolment can later introduce per-device keys + certificates).
- Publishing the agreement key as a converging op means #14 can resolve any
  member's encryption key from the projection alone — no key server.
- At-rest encryption is independent of identity (its own keystore key), so
  rotating one does not affect the other.
- The seed in a local file (fallback when no keystore is present) remains the
  weakest link; hardening that path is tracked in #6.

## Status of #6 at this ADR

Implemented: Ed25519 identity + signed ops, X25519 primitive + published
agreement key, secure key storage (keystore with file fallback), at-rest
encryption (#22). Deferred: the optional managed account (needs #20).
