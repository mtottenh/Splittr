# ADR-0001 — Event-sourced, CRDT-convergent core data model

- **Status:** Accepted
- **Date:** 2026-06-20
- **Deciders:** project owner + AI agent
- **Related:** Epic #18; issues #1, #15, #19, #8, #14. Transport is a separate decision (ADR-0002).

## Context

Splittr's advanced roadmap (multi-device + multi-user sync, offline-first, an
expense lifecycle/state machine, audit-grade history, end-to-end encryption)
cannot be served by the v1 model — a single mutable `AppState` snapshot,
mutated in place and rewritten as one JSON file. Those features all assume a
data model that is **local-first**, **convergent without a central authority**,
and **tamper-evident**.

The foundation must be correct *before* any value-add feature is built, because
every later feature reads and writes through it. This ADR fixes that foundation.

## Decision

**The source of truth is an append-only log of signed, immutable operations
("ops"). All readable state — groups, expenses, balances — is a deterministic
*projection* (a pure fold) over the op-log. Conflict resolution is defined per
entity so that any two replicas holding the same set of ops compute byte-for-byte
identical state, regardless of the order in which ops arrived (CRDT
convergence).**

The pure-Dart domain layer already shipped (`SplitCalculator`,
`BalanceCalculator`, `DebtSimplifier`, `Money`, the immutable models) **is** that
fold and is preserved verbatim. The imperative mutation/persistence core is
replaced.

### Why event sourcing + CRDT (vs a synced mutable DB)

An expense ledger is naturally **append-mostly and commutative**: expenses and
settlements are immutable facts; balances are a `reduce` over them. Modelling the
ledger as a set of signed ops makes "sync" nothing more than *set union + a
deterministic fold*. No server is required to compute the truth, offline edits
merge without coordination, and every op is independently verifiable.

## The operation model

```
Op {
  id:              OpId,            // BLAKE3(content) — content-addressed
  type:            OpType,
  payload:         <type-specific, see below>,
  group_scope:     GroupId?,        // routing, authorization, encryption-key selection
  hlc:             Hlc,             // (wall_ms, counter, site_id)
  author_device:   DevicePubKey,    // who signed it
  author_identity: IdentityPubKey,  // resolved via device cert (ADR/#16)
  sig:             Signature,       // device key signs the canonical encoding
}
```

- **Content-addressed `id` (BLAKE3 of the canonical bytes).** Gives free
  deduplication (the same op has the same id on every replica), tamper-evidence,
  and a natural key for set reconciliation. Uniqueness is guaranteed because the
  `hlc` (which includes `site_id` + a monotonic counter) is part of the hashed
  content.
- **Ops are immutable and never deleted.** "Editing" and "deleting" are
  themselves new ops (see lifecycle). The log is the audit trail.
- **Every op is signed** by a device key; verification chains the device to an
  identity via its device certificate (#16) and checks authorization for the op
  type (e.g. only a group member may post an expense to that group). Ops that
  fail signature/authorization are rejected at apply time and never affect state.

### Hybrid Logical Clocks (HLC)

Each op carries an HLC `(wall_ms, counter, site_id)`:

- **Local event:** `wall = max(now, last.wall)`;
  `counter = (wall == last.wall) ? last.counter + 1 : 0`.
- **On receive:** advance the local clock past the remote HLC before issuing the
  next local op (`wall = max(now, local.wall, remote.wall)`, counter reconciled).
- **Total order:** compare `wall`, then `counter`, then `site_id`.

HLCs give a causality-respecting deterministic order without synchronized
clocks. They are used only as **tiebreakers** in last-writer-wins registers — the
*set* of immutable facts is order-independent by construction.

## Operation vocabulary

| Domain | Ops |
|---|---|
| Identity (#6/#16) | `UpsertProfile`, `AddDeviceCert`, `RevokeDevice` |
| People (#2/#8) | `CreatePerson` (placeholder or claimed), `AddAlias` (claim/merge) |
| Group | `CreateGroup`, `SetGroupMeta` (LWW fields), `SetMembership` |
| Expense (#15) | `CreateExpense`, `AddExpenseVersion`, `VoidExpense`, `SetExpenseLock` |
| Settlement | `RecordSettlement`, `VoidSettlement` |
| Keys (#14) | `PublishGroupKey`, `WrapGroupKey`, `RotateGroupKey` |

## Conflict-resolution rules (the correctness core)

Convergence depends entirely on these being **pure, deterministic, and
order-independent**. They are the most heavily tested part of the system.

1. **Immutable facts → grow-only set, dedup by op id.** `CreateExpense`,
   `RecordSettlement`, `AddExpenseVersion`, `AddAlias`, key ops. They accumulate;
   they never conflict. The fold reads the *non-tombstoned* subset.

2. **Mutable scalars → Last-Writer-Wins register by HLC.** Group name, emoji,
   currency, `simplifyDebts`, edit policy: `SetGroupMeta` carries the field; the
   value with the highest HLC wins (tiebreak `site_id`).

3. **Membership → LWW per `(group, member)` register.** `SetMembership` sets
   `member|removed` for a pair; highest HLC wins. Chosen over an OR-Set because
   it is simpler, deterministic, and—critically for E2E key rotation (#14)—makes
   "who is currently a member" unambiguous. Concurrent add/remove resolves by
   HLC; document that a same-instant tie resolves by `site_id`.

4. **Expense edits → whole-version LWW.** Each `AddExpenseVersion` carries the
   *entire* field-set (amount, splits, payers, date, category, notes). The
   materializer selects the **single winning version = highest HLC** among an
   expense's non-tombstoned versions. We deliberately avoid field-level merge:
   merging two concurrent edits field-by-field can produce a split that no one
   entered and that may not even reconcile to the total. Whole-version LWW is
   deterministic and *always* yields a valid, balanced expense; the "losing"
   edit is preserved in history and surfaced as "edited on another device."

5. **Delete wins, and is terminal.** `VoidExpense`/`VoidSettlement` are
   monotonic tombstones: **once any replica observes a void for an entity, it is
   void on every replica after merge**, regardless of the HLC of any edit. This
   makes the rule "a concurrent edit-vs-delete resolves to delete" fall out
   naturally (money must not silently resurrect). Edits to a voided entity remain
   in the log for audit but never un-void it; re-adding is a *new* entity, never
   a resurrection.

6. **Lock is an LWW register + an authorization guard, not a CRDT axis.**
   `SetExpenseLock` records lock state (LWW); locking is enforced at command
   time (reject edits to locked entities) and surfaced on reconciliation. We
   intentionally do **not** let lock participate in version/void resolution — it
   would add concurrency corner-cases for little benefit.

7. **Aliases (claim/merge #8) → union-find to a canonical id.** `AddAlias` edges
   are immutable facts; resolution unions them and picks the canonical id
   deterministically: a claimed account always beats a placeholder; between two
   accounts, the lexicographically-lowest identity pubkey wins. The fold resolves
   every user id through the alias map *before* aggregating balances, so a claim
   changes attribution with **zero** change to any balance. Cycles are impossible
   given the canonical rule.

## Projection / materialization

- The **materializer** folds ops into Drift projection tables (#1). It is a pure
  function of the op *set*. Two equivalent paths must always agree:
  **incremental apply** (apply each op as it arrives) and **full rebuild**
  (drop projection, replay the whole log). This equivalence is an explicit test.
- Balances are computed by the existing domain services over the resolved
  (alias-mapped), non-tombstoned expense/settlement set.
- Projection tables are a disposable cache; the op-log is canonical.

## Storage (see #1)

- `ops` — append-only, content-addressed id, signature, **encrypted** payload
  (ADR-0002/#14) plus a locally-decrypted cache column; indexed by
  `(group_scope, hlc)`.
- Projection tables — `users`, `groups`, `group_members`, `expenses`,
  `expense_versions`, `expense_state`, `splits`, `payers`, `settlements`,
  `activities`, `user_aliases`. Every row carries UUID, `updated_at`,
  `deleted_at` tombstone, `hlc`, `site_id`.
- `hlc_state`, `sync_state` (watermarks for transport, ADR-0002).
- **No JSON document store remains** (#1): the v1 `FilePersistence` / whole-file
  snapshot is deleted with no legacy importer. Model-level JSON is *repurposed*
  as op-payload encoding only.

## Money & determinism invariants

- All money stays integer cents; splits reconcile exactly (largest-remainder),
  unchanged from v1.
- The fold must be deterministic across platforms: no reliance on map iteration
  order, wall-clock, or locale inside the fold. Sort by stable keys (op id, user
  id) everywhere.

## Testing strategy (non-negotiable)

1. **Property-based convergence (headline test):** generate a random set of ops,
   apply it in many random orders and partition/merge interleavings across N
   simulated replicas, assert **identical projections and balances**. This single
   guarantee is what the whole product rests on.
2. **Conflict-case tests:** concurrent edit/edit, edit/void, void/edit,
   membership add/remove races, alias merge with a mutual debt between the
   merged users.
3. **Rebuild equivalence:** incremental projection == replay-from-scratch.
4. **Idempotency:** applying an op twice is a no-op.
5. **Authorization/signature rejection:** forged or unauthorized ops never
   affect state.

## Consequences

**Positive:** offline-first and conflict-free by construction; complete audit
history; tamper-evident; transport-agnostic (ADR-0002 can choose/replace the
network layer without touching semantics); the valuable, tested domain layer is
reused as the fold.

**Negative / costs:** the op-log grows unbounded (mitigation: per-group
**snapshots/compaction** as a later optimization — fold a prefix into a
checkpoint, keep it verifiable); more moving parts than CRUD; developers must
think in commands+projections, not mutations.

**Explicitly deferred:** log compaction/snapshotting; field-level edit merge
(only if whole-version LWW proves too lossy in practice).

## Alternatives considered

- **Synced mutable SQL + last-write-wins rows:** simplest, but loses history,
  can't merge offline edits safely, and isn't tamper-evident. Rejected.
- **General-purpose JSON CRDT (Automerge/Yjs):** powerful, but heavier than
  needed and its generic merge can't express domain rules like "delete wins" or
  "valid split only." We need a *small* op-log with *our* fold. Rejected as the
  core (may still inform tooling).
- **Per-field LWW for expenses:** rejected in favour of whole-version LWW to
  guarantee always-valid splits (rule 4).
