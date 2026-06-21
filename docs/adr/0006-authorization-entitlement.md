# ADR-0006 — Fold-time authorization & entitlement

**Status:** Accepted · **Date:** 2026-06-21 · Supersedes the "deferred" scope
note in ADR-0005 · Issues: #38 (gates #14/#20), #7, #8.

## Context

ADR-0005 added device identity and made the fold enforce two things:
*authenticity* (`Op::verify` at ingestion) and *device revocation* (a revoked
device's ops stop counting). It explicitly deferred **entitlement** — "only an
identity that is allowed to may affect a group" — because a naive "require a
cert" check buys nothing (an attacker self-certifies a fresh key), and real
entitlement needs a trust anchor (group membership) plus convergent rules. The
gap is harmless single-device but is a forgery hole the moment sync (#14/#20)
ingests foreign ops, so it must close first.

## Decision

Authorization is a **pure, order-independent function of the whole op-set**,
computed in an `Authority` pass before the value fold (`splittr-crdt::materialize`).
An op folds only if its **author's identity is entitled** to it.

### Actor resolution
`op.author` (a device key) → identity via its certificate (default: itself,
self-sovereign) → `user_id_for` → alias-resolved to canonical (#8). This is the
op's **actor**.

### Group membership (the trust anchor)
- A group's **founder** is the actor of its *earliest* `CreateGroup` (by HLC then
  op-id); a later forged re-create can't seize the group or rename it (only the
  founder's `CreateGroup` name counts).
- Membership is a **time-sliced** per-(group, user) timeline of authorized
  `SetMembership` events, replayed in the `(HLC, op-id)` total order (a
  deterministic linearization). The founder is seeded as a member from the start
  of time. **Any member as of an op may add or remove members** (a communal,
  low-stakes v1 model). Because membership is time-sliced — like revocation — a
  removal only affects *later* ops; a removed member's past activity is preserved.

### Friendship (the non-group trust anchor, #7/#38)
`DeclareFriend{other}` is a self-signed, one-way statement of intent. A
**friend edge** exists between two identities only when **both** have declared
toward each other (a mutual, order-independent record folded into
`Projection.friends`). Like membership it is communal and convergent: the edge
is a deterministic function of the op-set, not of arrival order, and a
declaration toward an unclaimed `user:` **placeholder** never forms an edge
(there is no second party to reciprocate).

### Per-op entitlement
- `CreateGroup`: only the founder's counts. Group settings / closed period /
  group expenses / settlements / their dependents (edit/void/publish/lock):
  the actor must be a **member as of that op**.
- Non-group (friend) expense/settlement (#31): the actor must be a **participant**
  *and* must share a **mutual friend edge** with every other real party.
  Placeholder parties (`user:` guests, not yet a real identity) are exempt —
  you can record a split with a guest you've added locally, exactly as a group
  founder seeds members. This closes the v1 "participant-only" gap (#38b): a
  stranger can no longer attach an expense to you by self-certifying and naming
  you as a payer.
- `DeclareFriend`: owned by the `Authority` pass (it builds the friend edges);
  it carries no value and is ignored by the value fold.
- `UpsertProfile`: actor is the subject, or the subject is an unclaimed
  placeholder (anyone may name a guest). `SetAgreementKey`: actor is the subject.
- `AddAlias` (#8 claim/merge): the asserting identity must be a party to the
  merge, **or** both endpoints are placeholder guests (anyone may reconcile two
  guests — low-stakes, reversible). Canonical id per ADR-0001 rule 7 (a claimed
  account always beats a placeholder; else lowest pubkey).

Dependent ops resolve their context from the **authorized** creating op (an
expense/settlement with no authorized `Create*` has nothing to act on → dropped).

## Consequences

- The forgery hole closes for groups: an outsider's ops (membership self-grant,
  expense, settlement, foreign profile/key write) are dropped, order-independently
  and convergently. This is the gate that unblocks sync (#14/#20).
- **#7 (invites)** is partly in the engine now. *Group* admission still needs
  no new fold rule (any member may `add_member`). What the engine adds is a
  signed **invite token** (`splittr-crypto::Invite`): an identity-signed,
  domain-separated, expiring blob carrying `friend` or `group:<id>` context, for
  out-of-band sharing (link/QR). It is a **trust artifact for the shell**, not
  consumed by the fold — accepting a friend invite drives `add_friend`
  (`DeclareFriend`); the friend edge still requires both sides to declare. The
  remaining work is transport/peering (#9) and the accept UX.
- **#8 (claim/merge)** is implemented in the engine (`claim_person`,
  `merge_people` → `AddAlias`); the remaining work is shell UX (preview, undo,
  "is this you?").
- **#38b (friend-edge gating)** is now implemented: non-group entitlement is a
  mutual friend edge, not just a participant check. The follow-up it was waiting
  on (#7's accepted-friend record) is the `DeclareFriend` op above.
- **Known v1 simplifications (follow-ups):** membership is communal (any member
  may remove any member — no admin role beyond the founder seed); friendship is
  likewise communal and revocation of a `DeclareFriend` is not yet modelled;
  merging two *placeholder* guests is open to any actor. None weaken group
  balances; all are tracked for hardening before/with sync.

## Testing

The convergence property test generates an authorized world (founded group,
real-identity members, an outsider, forged certs, mutual friend edges) and
asserts identical projections across shuffles. Targeted `rules.rs` tests pin
each entitlement rule and its order-independence: non-member expense dropped,
self-grant ignored, post-removal ops dropped (past preserved), dependent ops on
an unauthorized expense dropped, foreign profile/key writes dropped, the
**friend edge needs both declarations**, a friend expense between non-friends is
dropped, a placeholder party needs no reciprocation, friendship is
order-independent and alias-aware, alias-claim authorization, and concurrent-claim
convergence. `splittr-crypto` property/behaviour tests pin the invite token
(create/verify, field tampering, expiry, round-trip encode/decode).
