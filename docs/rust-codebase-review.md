# Rust Codebase Review — `splittr-core`

> Principal-level engineering review of the Rust engine. Scope: ~6,500 LOC of
> hand-written code across 6 crates (`splittr-domain`, `splittr-crdt`,
> `splittr-crypto`, `splittr-store`, `splittr-app`, `splittr-ffi`); the generated
> `frb_generated.rs` was excluded. Toolchain (`cargo fmt --check`,
> `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`)
> was run and is green.

## 1. Executive summary

This is a **well-architected, genuinely high-quality codebase** — noticeably above
the average for a project at this stage. The hexagonal layering
(`domain → crdt → store → app → ffi`) is clean and the dependency direction is
correct (no cycles, leaf crates stay leaf). The event-sourced/CRDT core is
disciplined: a *single* LWW chokepoint (`materialize::lww`), content-addressed
dedup, and a property-tested convergence guarantee that is exactly the right
headline test. `fmt`, `clippy -D warnings`, and `test` all pass clean, and CI
enforces them plus a "clean-break" guard against the deleted v1 Dart layer. There
is **no `unsafe`**, no `todo!`/`unimplemented!` in shipping paths, and the panics
that exist are all genuinely-infallible `expect`s (OS RNG, postcard encoding,
BIP39 entropy).

Attention therefore went where the tools can't see. Four themes dominate:

1. **A documented capability that isn't actually implemented.** Device
   *authorization* is collected at cost but never enforced in the fold — only
   *revocation* is. `ARCHITECTURE.md`/ADR-0005 claim ops "count only while
   authorized," which the code does not do. A doc/impl mismatch today; a security
   gap once sync (#14/#20) lands.
2. **"Incremental" that isn't.** `Materializer`/`Repository` recompute the whole
   projection from scratch on every read, and `Projection::for_group` deep-clones
   the entire projection per group — so a single `groups()` call is O(n²) in
   clones. The naming and doc-comments oversell this as incremental/kept-in-sync.
3. **Crypto-format durability shortcuts.** `Argon2::default()` params are unpinned
   and the at-rest/vault blobs carry no version byte — a dependency bump can render
   all existing encrypted data undecryptable, with no migration path. Secret
   material isn't zeroized despite `zeroize` already being in the tree.
4. **A pocket of API debt** around expense creation: six near-identical methods,
   each 8–9 positional args, each silencing `clippy::too_many_arguments`.

None of these are memory-safety/soundness bugs. There are **no Critical findings** —
stated plainly rather than manufactured.

## 2. Findings by severity

### High

#### H1 — Device authorization is collected but never enforced; only revocation gates ops
- **Location:** `crates/splittr-crdt/src/materialize.rs:88-93` (`Devices::op_authorized`),
  feeding `fold` at `:391-400`; behavior enshrined by `crates/splittr-crdt/tests/rules.rs:421`.
- **What.** `collect_devices` builds an `authorized` map (`device → (hlc, identity, site)`),
  but `op_authorized` consults only `revoked`:
  ```rust
  fn op_authorized(&self, op: &Op) -> bool {
      match self.revoked.get(&op.author) {
          Some(revoke_hlc) => op.hlc < *revoke_hlc,
          None => true,                 // ← any non-revoked author counts
      }
  }
  ```
  The `authorized` set is used *only* to populate the `devices` projection for
  display — it never gates whether an op folds. The integration test makes this
  explicit: *"With no (valid) cert the device acts as its own identity, so its op counts."*
- **Why it matters.** `ARCHITECTURE.md` §7 and ADR-0005 state: *"a device's ops
  count only while it is authorized and not yet revoked."* The code does not
  implement that — an op signed by **any** valid keypair counts toward balances
  unless that exact key is later revoked. While the system is single-device this is
  harmless, but the moment sync (#14/#20) accepts ops from the network, an attacker
  with a fresh keypair injects ops that affect balances, and revocation can't help
  (each forgery uses a new key). Separately, `collect_devices` does a full O(n) pass
  to build a map that is currently dead weight for the fold (a DRY/dead-logic smell).
- **Suggested fix.** Either (a) make `op_authorized` actually require authorization —
  an op counts only if `author` is in `authorized` (resolving the device→identity
  binding) and that identity is entitled (e.g. a group member) — *or* (b) if the
  permissive "self-enrolling first device" model is the real intent for now, correct
  `ARCHITECTURE.md`/ADR-0005 to describe it accurately and add a
  `// TODO(#14): gate on authorization before sync` at the `None => true` arm. Right
  now the docs and the code disagree, which is the worst of both worlds.

#### H2 — The projection is recomputed from scratch on every read; `for_group` deep-clones per group
- **Location:** `crates/splittr-crdt/src/materialize.rs:66-68` & `:391`;
  `crates/splittr-store/src/repository.rs:55-57`;
  `crates/splittr-crdt/src/projection.rs:39-58`;
  consumed in `crates/splittr-app/src/query.rs:217-231` (`groups`).
- **What.** `Materializer::projection()` calls `fold(self.ops.values())`, which makes
  **two full passes** over every retained op (`collect_devices` then the fold),
  cloning every field into a fresh `Projection` — *on every call*. `Repository` holds
  a `Materializer` but caches nothing; `Repository::projection()` re-folds the entire
  log each time. On top of that, `Projection::for_group` clones `groups`, `users`,
  `aliases`, and `devices` in their entirety, and `query::groups` calls it once
  *per group*:
  ```rust
  p.groups.iter().map(|(id, group)| {
      let net = net_balances(&p.for_group(id)); // full-projection clone, per group
      ...
  })
  ```
  So one `groups()` render is O(groups × whole-projection). `App::activity()` is even
  heavier: it decrypts/deserializes the entire op-log (`store().ops()`) **and**
  re-folds it.
- **Why it matters.** The FFI typically queries after every command, so a session is
  O(n²) in folds and clones over the op-log. The doc-comments actively mislead:
  `repository.rs:2` says *"in-memory projection kept up to date as ops are appended"*
  and `materialize.rs:46` advertises an "incremental" path — but there is no cached
  projection and no incremental application; it's refold-everything. Folding the whole
  set (for order-independent revocation) is legitimate; recomputing it K times
  *between* appends is not.
- **Suggested fix.** Cache the `Projection` in `Repository`, invalidate on a
  *successful* `append`, recompute lazily at most once per change:
  ```rust
  pub struct Repository<S> { store: S, materializer: Materializer, cached: RefCell<Option<Projection>> }
  // append(): on Applied::Stored, *cached.borrow_mut() = None;
  // projection(): borrow-or-recompute-and-store.
  ```
  Make `query::groups` fold **once** (`net_balances` over the full projection, then
  bucket by group) instead of `for_group` per group, and drop or sharply scope
  `for_group`'s cloning. Update module docs to say "recomputed on demand," not "kept
  up to date," until a cache exists.

#### H3 — Unpinned Argon2 params and unversioned crypto blob formats risk permanent data loss
- **Location:** `crates/splittr-crypto/src/vault.rs:44-50` (`derive_key`) and the blob
  layouts in `vault.rs` (`salt(16) || …`) and `aead.rs:24-46` (`nonce(24) || …`).
- **What.** The root-seed vault derives its key with `Argon2::default()`. Argon2
  parameters (memory/time/lanes) are part of the derivation, but `default()` is
  whatever the `argon2` crate version ships — not pinned in code. Neither the vault
  blob nor the AEAD blob carries a **format/version byte**.
- **Why it matters.** If the `argon2` crate ever changes its default params (routine as
  hardware advances), `derive_key` produces a *different* key from the same
  passphrase+salt, and **every previously-sealed root seed becomes undecryptable** —
  users lose their identity with no recovery path other than the BIP39 phrase. With no
  version byte, you also can't migrate formats or rotate AEAD schemes without a flag
  day. Classic at-rest-crypto corner-cut: works today, breaks silently on a dep bump.
- **Suggested fix.** Pin explicit Argon2 params via `Argon2::new(...)` (treat changing
  them as a versioned migration), and prefix both blob formats with a 1-byte version
  tag that `open`/`open_seed` switch on:
  ```rust
  let params = Params::new(19_456, 2, 1, Some(32)).expect("valid argon2 params");
  let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
  // blob = [VERSION_1, salt.., sealed..]
  ```

### Medium

#### M1 — Secret material is not zeroized, though `zeroize` is already in the dependency tree
- **Location:** `crates/splittr-crypto/src/aead.rs:14-21` (`AeadKey([u8;32])`, derives
  `Clone`); raw `[u8;32]` seeds returned from `vault.rs:33` (`open_seed`) and
  `recovery.rs:18` (`seed_from_phrase`); the `key` array in `vault.rs:45`; the seed
  parameter in `identity.rs:83` (`unlock`).
- **What.** `ed25519_dalek::SigningKey` and `x25519_dalek::StaticSecret` zeroize
  themselves on drop, but the crate's *own* secret-bearing types don't: `AeadKey` is a
  plain `[u8;32]` (and `Clone`, so copies proliferate), and seeds are passed/returned
  as bare arrays that linger in memory.
- **Why it matters.** This is a security-sensitive, device-theft-threat-model app;
  leaving key material in freed memory undercuts the at-rest story. Fix cost is
  near-zero — `zeroize 1.9` is already in `Cargo.lock` (pulled by dalek).
- **Suggested fix.** `#[derive(Zeroize, ZeroizeOnDrop)]` on `AeadKey`, wrap the derived
  `key` and returned seeds in `Zeroizing<[u8;32]>`, and drop `Clone` from `AeadKey` (or
  make copies explicit).

#### M2 — Money arithmetic is unchecked
- **Location:** `crates/splittr-domain/src/money.rs:20-47` (`Add`/`Sub`/`Neg`/`AddAssign`/`SubAssign`).
- **What.** `Cents` operators use raw `+`/`-`/`-`, which panic on overflow in debug and
  wrap in release.
- **Why it matters.** The whole point of `Cents` is "money is exact." A wrapping balance
  is a silent correctness failure precisely in the type meant to prevent them. Realistic
  cent totals won't overflow `i64`, so this is Medium not High — but the invariant should
  be enforced or explicitly documented, not left implicit.
- **Suggested fix.** Either document the "values fit in i64" invariant on the type, or
  provide `checked_add`/`saturating_add` and use them on aggregation paths
  (`net_balances`, `total_paid`). At minimum, a `# Panics` note.

#### M3 — Expense creation: six near-identical methods, 8–9 positional args each, six `#[allow(too_many_arguments)]`
- **Location:** `crates/splittr-app/src/app.rs:202,230,259,288,326,607` (the `#[allow]`s)
  and the `add_expense`/`add_draft_expense`/`add_non_group_expense`/`create_expense`/
  `edit_expense`/`build_fields` cluster.
- **What.** Each lint suppression signals the API shape is wrong, not that the lint is.
  The same 8 fields (`description, paid_by, total, split, category, notes, date_ms,
  original`) are threaded positionally through six functions. The FFI layer **already**
  has the right abstraction — `ExpenseInput` (`dto.rs`) — but the app layer doesn't.
- **Why it matters.** Positional args of the same types (`&str, &str` for
  description/category; two `Option`s) are easy to transpose silently, and the
  duplication makes signature changes a six-site edit.
- **Suggested fix.** Introduce a params struct in `splittr-app` (e.g.
  `ExpenseDraft { fields…, group: Option<GroupId>, draft: bool }`), collapse the wrappers
  to thin constructors over it, and delete all six `#[allow]`s. `build_fields` then takes
  the struct.

#### M4 — `RedbOpStore::len()` decrypts and deserializes every op just to count them
- **Location:** `crates/splittr-store/src/redb_store.rs:101-105`.
- **What.** `len()` is implemented as `self.ops()?.len()` — a full decrypt + postcard
  decode of the entire table to produce a count, with a comment that this avoids redb's
  moving metadata trait.
- **Why it matters.** `is_empty()` (used at open) and any future `len()` caller pay O(n)
  crypto work for a number redb can give in O(1). The "metadata trait moved" concern is
  real but the current redb (`2.x`) exposes `Table::len()`.
- **Suggested fix.** Use `read.open_table(OPS)?.len()` and map the error; keep the decode
  path only for `ops()`.

#### M5 — `money()` formatter ignores currency minor units
- **Location:** `crates/splittr-app/src/query.rs:316-319`, used in `activity` (`:346`).
- **What.** `money()` always formats as `cents/100 . cents%100`, hardcoding 2 decimals —
  but the codebase explicitly models 0- and 3-decimal currencies (`currency::minor_units`,
  JPY=0, BHD=3).
- **Why it matters.** A ¥1000 settlement renders as "10.00" and a BHD amount is off by a
  factor of 10 in the activity feed. Display-only (balances are correct), hence Medium, but
  it duplicates formatting logic that should defer to `minor_units` (a DRY + correctness
  miss in the same spot).
- **Suggested fix.** Thread the relevant currency into `money()` and format against
  `minor_units(code)`; or move money formatting into one shared helper used by both Rust
  and the shell.

#### M6 — `rate_micro as u64` can silently wrap a negative input
- **Location:** `crates/splittr-ffi/src/api.rs:409` and `crates/splittr-ffi/src/convert.rs:34`.
- **What.** `rate_micro` crosses the FFI as `i64`, then `as u64`. A negative value wraps to
  a huge magnitude rather than being rejected.
- **Why it matters.** Untrusted input at the boundary feeding money conversion; a wrap
  produces a wildly wrong converted amount instead of a clean error.
- **Suggested fix.** `u64::try_from(rate_micro).map_err(|_| anyhow!("rate must be non-negative"))?`
  at the FFI edge.

#### M7 — `Op::verify` returns `bool`, discarding the failure reason
- **Location:** `crates/splittr-crdt/src/op.rs:153-156`; caller `repository.rs:44`.
- **What.** The trust-boundary check collapses "content-hash mismatch" and "bad signature"
  into a single `false`, which `Repository::append` turns into a generic `Applied::Rejected`.
- **Why it matters.** At the one place authenticity is enforced, you lose the ability to
  distinguish corruption from forgery for logging/telemetry — useful once ops arrive over
  the network.
- **Suggested fix.** Return `Result<(), VerifyError>` with `HashMismatch`/`BadSignature`
  variants; `append` can still map both to `Rejected` while logging the cause.

### Low / Polish

- **`hex32` reinvented three times** (`query.rs:19`, `api.rs:383`, `identity.rs:146`), each
  looping with `format!("{b:02x}")` (a heap allocation per byte). The `hex` crate is already
  in the tree (transitively); otherwise hoist one shared `fn hex32(&[u8;32]) -> String`.
  (DRY + a minor hot-path allocation.)
- **`Cents` lacks `Display`**, which is partly why `money()` is hand-rolled. A
  `Display`/formatting helper on the type would centralize money rendering.
- **`assemble` membership loop is O(groups × membership)** (`materialize.rs:286-297`): for
  each group it scans the entire `membership` map. Build a `group → members` map in one pass.
- **`ids.rs` macro** provides `From<&str>` and `new(impl Into<String>)` but not
  `From<String>`; the two construction paths overlap. Consider `From<String>` for symmetry.
- **Dependency hygiene:** three `getrandom` majors (0.2/0.3/0.4) coexist (transitive); the
  workspace pins 0.2 directly. **`cargo-audit`/`cargo-deny` is not in CI** — cheap insurance
  for a crypto-bearing app.
- **`from == to` settlement guard** (`app.rs:426`) compares raw `UserId`s without alias
  resolution; a self-settlement across two aliased ids would slip through. Edge-case, post-#8.

## 3. Remediation plan

**Quick wins (hours, low risk):**
1. Cache the projection in `Repository` and invalidate on append — biggest perf return for
   the least change (**H2**, part 1).
2. Pin Argon2 params + add version bytes to both crypto blobs (**H3**).
3. `RedbOpStore::len()` → `table.len()` (**M4**); `try_from` on `rate_micro` (**M6**).
4. Zeroize `AeadKey`/seeds (**M1**); add `cargo-audit` to CI; unify `hex32` (Low).
5. Reconcile the device-authorization docs with the code, or add the `TODO(#14)` marker
   (**H1**, decision-pending half).

**Larger refactors (sequence deliberately):**
6. **Decide and implement the authorization model** before sync work begins — make
   `op_authorized` enforce the `authorized` set, or formally adopt the permissive model in
   the ADR (**H1**). Settle this first because #14/#20 build on it.
7. Collapse expense creation onto a params struct and delete the six `#[allow]`s (**M3**).
8. Refold `query::groups`/`for_group` to a single fold + bucketing (**H2**, part 2).
9. Centralize money formatting against `minor_units` and add `Cents: Display` (**M2/M5**),
   then make arithmetic checked on aggregation paths.

---

**On what's right**, since it's most of the code: the convergence/rules property tests
(`tests/common/mod.rs`, `convergence.rs`, `rules.rs`) are the real asset — deliberately
misaligned HLC vs arrival order, a fixed entity universe to force conflicts, reproducible
shuffles. Keep that bar; it's what makes the refactors above safe to attempt.
