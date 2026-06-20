# ADR-0005 — Device identity: separating identity from devices

- **Status:** Accepted (2026-06-20)
- **Date:** 2026-06-20
- **Deciders:** project owner + AI agent
- **Supersedes:** the device-layer notes in ADR-0004 (which deferred device keys to "#16, later"). ADR-0004's key *hierarchy* still holds; this ADR fixes the *device* layer.
- **Related:** #16 (multi-device), #9/#20 (sync), #14 (E2E), #22 (at-rest), #6 (identity primitives).

## Context

Today every op is signed **directly by the identity key** (single-device "Model 0"). That key would have to be copied to every device, which means: one device compromise permanently compromises the identity, there is **no revocation** (you can't disown a lost phone without changing the user id and orphaning all history), and there is no per-device attribution. Sync (#20) and E2E (#14) make this untenable — E2E wraps content keys *per recipient device*, and revocation is meaningless if the key is identical everywhere.

We are **decentralized-first**: there is no CA. So identity must be **self-certifying** (the public key *is* the user id) and device↔identity binding must be a **signed statement**, verified cryptographically; the human↔identity binding is established **out-of-band** (invites/QR, #7).

## Decision

A three-level hierarchy, with device bindings expressed **as ops in the same signed log** (no separate PKI):

```
recovery phrase (BIP39) ──derives──▶ Identity (root) key (Ed25519) = stable user id   [used only to enrol/revoke]
                                          │ signs
                                          ▼
                                  AuthorizeDevice op { identity, device, site }   (an op, signed by the root)
                                          │ authorizes
                                          ▼
  per device:  Device key (Ed25519) ── signs every *normal* op
               Device X25519 key     ── recipient for E2E key-wrapping (#14)
```

### Ops carry the device binding
- `AuthorizeDevice { identity, device, site }` — **signed by the identity (root) key**. Establishes `device → identity`. Accepted only when `op.author == identity` (the root authorises its *own* devices). Root-only cert signing (no delegated enrolment for v1).
- `RevokeDevice { identity, device }` — signed by the identity key; **terminal**.
- Every *other* op is **authored/signed by a device key**.

### Two-layer trust check (preserves CRDT convergence)
- **Authenticity** (ingestion, `Op::verify`): the op's signature matches its `author`. Always checkable, order-independent. Forged ops rejected here. *Unchanged.*
- **Revocation** (the fold, pure over the whole op-set): a normal op is counted **iff** its author device is not revoked **as of that op's HLC**. Computed from the `AuthorizeDevice`/`RevokeDevice` op-set, so it is order-independent and convergent. Ops authored *before* a device's revocation stay valid; ops *after* are dropped. A key with no device cert resolves to **itself** as identity, so existing single-key data keeps working (graceful, non-breaking).

> **Scope note (corrected).** Only *authenticity* and *revocation* are enforced in the fold today. This layer does **not** yet enforce **entitlement** — that the author's identity is actually a *member* allowed to affect the touched group. Because anyone can self-certify a fresh key (`AuthorizeDevice{K, K}` signed by `K`), requiring "a cert" buys nothing without a real trust anchor; entitlement needs the invite/membership layer (#7/#8) and an out-of-band-established member set. It is therefore **deferred and must land before sync (#14/#20)** accepts foreign ops. Until then the engine trusts locally-produced ops. Tracked in **#38**. (Earlier drafts of this ADR said ops "count only while authorized," which overstated what the fold enforces; this note is the correction.)

### Balances aggregate by identity
The fold resolves `device → identity` (default: self) — the same shape as alias resolution. User ids stay the identity key's fingerprint (`id:<hex>`), unchanged.

### Root key storage & recovery (decided)
- **Encrypted-on-primary + phrase backup.** The root key is derived from a **BIP39 recovery phrase** shown once at setup; it is also stored **encrypted on the primary device** (unlocked by the app lock / biometric, #22) so routine enrol/revoke doesn't require re-typing the phrase. The phrase is the cold, server-free recovery path ("new phone, lost old one").
- Daily ops never touch the root key — they use the device key.

### Enrolment & revocation UX
- **Add a device:** Signal-style — the existing (primary) device shows a QR (ephemeral channel); the new device generates its own keys and sends its pubkey; the primary signs an `AuthorizeDevice` op; both show a **short authentication string** to defeat MITM.
- **New phone (lost old):** restore from the recovery phrase → re-derive root → enrol the new device → `RevokeDevice` the old one.
- **Revoke:** any device with root access; for E2E this **rotates the group key** (#14) for forward secrecy.

## Consequences
- Per-device attribution + revocation without ever changing the user id.
- The root key is used rarely and kept cold-ish (encrypted, phrase-recoverable), shrinking its exposure.
- Device certs are just ops → they converge, are content-addressed, and need no separate distribution channel.
- The fold gains a whole-set authorization pass (revoked-device filtering); the materializer folds the **authorized** op-set. Convergence is preserved and property-tested.
- Backward-compatible: keys with no device cert act as their own identity, so pre-existing data and tests keep working.

## Sequencing
This is the **first milestone of Phase 1**, *before* transport (#20) and E2E (#14): changing the op author/signature model after real signed data syncs between people would be a trust-boundary migration we refuse to do. See #18 / #16 and the new sub-issues.
