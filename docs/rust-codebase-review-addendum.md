# Rust Codebase Review — Addendum: cross-cutting & system-level aspects

> Companion to [`rust-codebase-review.md`](./rust-codebase-review.md). That report
> covered the engine's *code*; this note covers the aspects that sit **above** the
> code — the persisted/signed wire format, the FFI boundary, the Dart shell and the
> presentation-only invariant, observability, supply chain, and determinism. Each
> section states what was checked and the verdict, so "reviewed and healthy" is
> recorded as explicitly as "needs work."
>
> Severities continue the first report's scale (Critical / High / Medium / Low).

## Contents
1. Op-format schema evolution & wire-format stability — **High**
2. FFI boundary: panics, mutex poisoning, codegen drift — **Medium**
3. The Dart shell & the "presentation-only" invariant — **Medium** (mostly healthy)
4. App-lock, vault & the at-rest story end-to-end — **High / Medium**
5. Observability — **Medium**
6. Supply chain, advisories & MSRV — **Medium / Low**
7. Determinism audit — **reviewed, healthy**
8. Not yet reviewable (sync / E2E)

---

## 1. Op-format schema evolution & wire-format stability — High

**The single most expensive-later, cheapest-now issue in the system, and the one
the first report under-covered.**

The `Op` / `OpKind` / `ExpenseFields` types are serialized with **postcard**, which
is a **non-self-describing** format: enum variants are encoded by their *ordinal
index*, struct fields by *declaration order*, with no field names, no tags, and no
unknown-field tolerance. That encoding is load-bearing in three compounding places:

- **`canonical()`** — `crates/splittr-crdt/src/op.rs:161-163` — postcard bytes are
  the input to **both** the BLAKE3 content-addressed `OpId` **and** the Ed25519
  signature.
- **The store** — `crates/splittr-store/src/redb_store.rs:50-65` — postcard is the
  persisted (and at-rest-encrypted) on-disk form.

**Why it matters.** Postcard's compactness is the right call for a content-addressed
log, but it makes the schema *brittle by construction*:

- **Reordering or inserting an `OpKind` variant** (the enum currently has 18 variants,
  `op.rs:18-117`) silently reassigns every later variant's index. Old stored/synced
  ops then deserialize as the *wrong* variant — or fail outright.
- **Adding a field to `ExpenseFields`** (`expense.rs:14-32`) breaks decoding of every
  previously-stored expense op.
- Most insidiously: because the bytes feed the id and signature, the *same logical op*
  encoded by an old vs new client gets a **different `OpId` and a non-verifying
  signature**. For a CRDT whose entire identity, dedup, and convergence model rests on
  content-addressing, a format change is a convergence-breaking, signature-invalidating
  event — not a routine migration.

There is currently **no format version byte, no migration path, and no golden/round-trip
test pinning the bytes**. Nothing would catch an accidental reordering in review; it
would surface as corrupt data or failed sync in the field.

**Suggested fix (do this before v1 has real data):**
- **Freeze the op wire format** and treat it as an API. Add `#[serde(...)]` discipline
  or, better, assign explicit variant discriminants and document "append-only, never
  reorder, never renumber" at the top of `OpKind`.
- Add a **golden-bytes test**: serialize a fixed `Op` of each variant and assert the
  exact hex, so any wire change fails CI loudly. (This is the determinism analogue of
  the convergence proptest — it guards the *format*, not the *fold*.)
- Decide a **versioning strategy** now: either a `version: u8` discriminator carried
  *outside* the signed `canonical()` region (so it can evolve without re-hashing), or a
  documented rule that new fields are only ever added via *new op variants*, never by
  changing existing ones. The latter fits event-sourcing best — old ops stay valid
  forever; new capability = new op kind.
- This is the same class of problem as **H3** (unversioned crypto blobs) in the first
  report; fix them together with a shared "everything persisted carries a version tag"
  convention.

---

## 2. FFI boundary: panics, mutex poisoning, codegen drift — Medium

Three distinct concerns; one is already mitigated, two are real.

**2a. Panic unwinding across FFI — mitigated (verified).** The generated bindings route
every call through `FLUTTER_RUST_BRIDGE_HANDLER.wrap_normal::<...>` (e.g.
`frb_generated.rs:56`), and FRB's handler wraps execution in `catch_unwind`, turning a
Rust panic into a Dart exception rather than UB. So the classic "panic across the
`extern "C"` boundary" hazard is handled by the framework. Good — no action needed, but
worth keeping in mind that it relies on FRB's contract.

**2b. Mutex poisoning permanently bricks the engine — real (Medium).**
`crates/splittr-ffi/src/api.rs:369-371`:
```rust
fn lock(&self) -> std::sync::MutexGuard<'_, App<RedbOpStore>> {
    self.inner.lock().expect("engine mutex poisoned")
}
```
Because 2a catches panics *but the panic still poisons the `Mutex`*, the sequence is:
a single panic inside any locked call (e.g. the unchecked money arithmetic flagged in
the first report's **M2**) → FRB reports one Dart exception → **every subsequent engine
call then panics on `.expect("engine mutex poisoned")`**, FRB converts those to
exceptions too, and the app is dead for the rest of the session with no recovery short
of restart. For a local-first app holding the user's only copy of their data behind that
lock, that's a poor failure mode.
- **Fix:** recover from poisoning instead of propagating it —
  `self.inner.lock().unwrap_or_else(|e| e.into_inner())` — or switch to
  `parking_lot::Mutex`, which doesn't poison. Combined with making the panic-prone
  arithmetic non-panicking (M2), this removes the brick scenario entirely.

**2c. Codegen freshness is not verified in CI — real (Low/Medium).** The bindings in
`lib/src/rust/` are committed generated code, and they currently *are* in sync (all 43
`pub fn`s in `api.rs` have camelCase Dart counterparts in `api.dart`; files share a
commit timestamp). But CI only runs `cargo build -p splittr-ffi` + `flutter test` — it
never runs `flutter_rust_bridge_codegen generate --check` (or regenerates and diffs). So
if someone edits `api.rs` and forgets to regenerate, CI stays green while Dart and Rust
silently diverge until a runtime "function not found."
- **Fix:** add a CI step that regenerates the bindings and fails on a non-empty
  `git diff` — the same pattern the repo already uses for the "v1 JSON store is gone"
  guard.

---

## 3. The Dart shell & the "presentation-only" invariant — Medium (mostly healthy)

I audited `lib/` against the non-negotiable invariant *"Flutter is presentation only;
business logic belongs in Rust."* **The invariant largely holds** — this is a genuinely
thin shell:

- `state/providers.dart` is a clean async facade: every mutation calls the engine and
  re-snapshots; the doc-comment ("Business logic lives in Rust — this is a thin async
  facade") is accurate.
- `add_expense_screen.dart` (the largest file, 793 LOC) builds a `SplitPlanDto`
  (`_buildPlan`, `:221`) and `Payer` list (`_buildPayers`, `:207`) and hands them to the
  engine — the authoritative split math runs in Rust. The local `totalCents ~/
  _participants.length` (`:647`) is a *display preview* and the `payers.fold(... ) !=
  totalCents` check (`:299`) is a *UX pre-validation*; neither is authoritative. Both are
  legitimate presentation concerns.
- `exchange_rate_service.dart` correctly keeps the conversion *maths* in Rust
  (`convertCurrency`) and limits itself to fetching a rate.

**The leaks worth flagging:**

**3a. Money parsing duplicates domain knowledge and hardcodes 2 decimals (Medium).**
`lib/core/money.dart` — `tryParseToCents` does `(value * 100).round()`, baking in "100
minor units per major unit." That is currency-specific domain knowledge the engine
already owns: `currencyMinorUnits(code)` is exposed over the FFI (and *is* called
elsewhere, `providers.dart:94`), but the parser ignores it. For JPY (0 decimals) or
BHD/KWD (3 decimals), `"1000"` yen is parsed as 100000 minor units — off by 100×. This
is the Dart twin of the first report's **M5** (`query.rs money()` ignoring minor units):
the same bug, duplicated on both sides of the boundary. `format` is safer (it delegates
to `NumberFormat.simpleCurrency`, which is locale/currency-aware), but parse and format
should both route through the engine's `minor_units`.

**3b. FX adapter has no request timeout (Low).** `HttpExchangeRateService.rate`
(`exchange_rate_service.dart:32`) issues `_client.get(uri)` with no timeout; a hung
connection blocks the future indefinitely (errors are otherwise handled gracefully →
`null`). Add `.timeout(...)`.

Net: the architectural boundary is respected; the issues are duplicated *formatting/parsing*
logic, not leaked *business* logic.

---

## 4. App-lock, vault & the at-rest story end-to-end — High / Medium

Tracing the encryption-at-rest claim across both languages surfaces gaps the per-crate
review couldn't see, because they live in the *seams*.

**4a. The file-fallback defeats at-rest encryption (High).**
`lib/state/engine.dart:111-119` (`SecretStore.write`) and `:93-109` (`read`): when no OS
keystore/secret-service is available (e.g. a headless Linux desktop — explicitly a
supported scenario), the identity seed, device seed, **and the database encryption key**
are written as plaintext files (`identity.seed`, `device.seed`, `db.key`) in the *same
application-support directory* as the encrypted `splittr.redb`. The db is still
"encrypted," but its key sits in a clear file right next to it — so for the device-theft
threat model the encryption provides ~zero protection on those platforms. The comment
acknowledges hardening is tracked under #6, but the current behavior should be stated
plainly: **on a keystore-less platform, at-rest encryption is defeated.** At minimum,
derive the file-fallback key from the OS user login / a passphrase rather than storing
it adjacent in clear.

**4b. The app-lock PIN uses a fast hash, inconsistent with the Argon2id used in Rust
(Medium).** `lib/state/app_lock.dart:11` — `hashPin = sha256("$salt:$pin")`, a single
SHA-256 round. The salt defeats precomputation but not brute force: a 4–6 digit PIN is
10⁴–10⁶ candidates, cracked in milliseconds if the stored hash leaks. Meanwhile the Rust
side already uses **Argon2id** for exactly this kind of low-entropy secret
(`crates/splittr-crypto/src/vault.rs`). Two different KDF philosophies for two PIN-shaped
secrets is both a security weakness and a consistency smell.

**4c. The app-lock and the vault are parallel, unwired mechanisms (Medium → architectural).**
There are effectively **two** PIN/passphrase systems that don't connect:
- the **Dart app-lock** (SHA-256 PIN) — gates the *UI* lock screen only; and
- the **Rust vault** (`seal_seed`/`open_seed`, Argon2id) — seals the *root seed*.

Critically, **the app-lock does not protect the database key** — that key lives in the
keystore (or the 4a plaintext file) and is loaded regardless of lock state. So today the
"app lock" is a presentation gate, not a cryptographic one: someone who bypasses the
Flutter lock screen (or reads the DB file directly) is not stopped by the PIN.
`ARCHITECTURE.md` §7 honestly notes "wiring that re-keying to the app lock is the
remaining tail" — this section just makes the current security posture explicit so it
isn't mistaken for more than it is. **Recommendation:** converge on one KDF (Argon2id,
already present) and actually derive/seal the db key under the app-lock secret, so the
lock has cryptographic teeth — and so 4a and 4b are fixed by the same change.

---

## 5. Observability — Medium

There is **zero** logging, tracing, or instrumentation anywhere in the core (verified:
no `tracing` / `log` / `eprintln!` / `println!` / `dbg!` under `crates/*/src`). For the
current pure-fold engine this is defensible — determinism means behavior is fully
reproducible from the op-log. But two things make it worth addressing soon:

- The **transport/sync work (#9/#20)** will be effectively undebuggable without
  structured tracing across the ingest → verify → fold → broadcast path. Distributed
  convergence bugs are exactly the kind you cannot reproduce by staring at code.
- Even today, a rejected op (`Applied::Rejected`) vanishes silently — see the first
  report's **M7** (`Op::verify` returns `bool`). There's no way to know *why* an op was
  dropped.

**Fix:** adopt `tracing` with spans at the use-case (`App::commit`) and trust
(`Repository::append`) boundaries before the sync crate lands; keep the pure fold
itself instrumentation-free to preserve determinism.

---

## 6. Supply chain, advisories & MSRV — Medium / Low

- **No `cargo-audit` / `cargo-deny` in CI (Medium).** For a crypto-bearing, soon-to-be-
  networked app, an unpatched advisory in `ed25519-dalek`, `chacha20poly1305`, `redb`, or
  the transitive tree would go unnoticed. `cargo audit` (and ideally `cargo deny` for
  license + duplicate-version policy) is a cheap CI addition.
- **No MSRV declared (Low).** No `rust-version` in the workspace or any crate manifest, so
  "supported Rust version" is undefined and CI silently tracks whatever `stable` is that
  week. Declare `rust-version` and pin a toolchain (`rust-toolchain.toml`) for
  reproducible builds — important once this ships to multiple platforms.
- **Duplicate transitive versions (Low).** Three `getrandom` majors (0.2/0.3/0.4) coexist
  in `Cargo.lock` (the workspace pins 0.2 directly). Harmless today; `cargo deny` would
  keep it from sprawling.

---

## 7. Determinism audit — reviewed, healthy ✅

Because the architecture stakes convergence on a deterministic fold, I specifically
checked for hidden non-determinism — and found none:

- **No `HashMap`/`HashSet` anywhere in the core** (verified across `crates/*/src`).
  Everything in the fold and projection uses `BTreeMap`/`BTreeSet`, so iteration order is
  deterministic by key — exactly right.
- **No floating-point in the money/fold paths.** All money is integer `Cents`; the only
  `f64` is in the Dart FX *input* adapter, and the conversion arithmetic is integer i128
  in Rust (`currency::convert`).
- **No wall-clock / locale dependence inside the fold.** `SystemTime` is confined to
  `HlcGenerator::now` (`app/src/clock.rs`), which only *stamps* ops; the fold consumes the
  recorded HLC, never the clock. Alias canonicalization uses lexicographic `min`
  (`materialize.rs:434`), not iteration order.

This is the system's strongest area and the property tests (`convergence.rs`, `rules.rs`,
`common/mod.rs`) back it with randomized orderings. The one addition I'd make is the
**golden-bytes wire-format test from §1** — convergence guards the *fold*; nothing yet
guards the *format* the fold depends on.

---

## 8. Not yet reviewable (sync / E2E)

The `splittr-sync` crate does not exist, and end-to-end encryption (#14) / key rotation
on member removal are unbuilt. So transport security, the zero-knowledge-relay claim, and
rekey-on-removal cannot be audited against code yet. They are noted here only because they
are the *reason* the first report's **H1** (enforce device authorization in the fold) and
this note's **§1** (freeze the wire format) should be settled *first*: both are
foundations the sync work will build on, and both are far cheaper to fix before peers are
exchanging signed ops over the wire.

---

## Consolidated priority across both documents

**Settle before any sync/transport work:**
1. Enforce-or-document device authorization (report **H1**).
2. Freeze + version + golden-test the op wire format (**§1**).
3. Version the at-rest/vault crypto blobs + pin Argon2 params (report **H3**).

**Quick wins:**
4. Recover from mutex poisoning / non-panicking money arithmetic (**§2b** + report M2).
5. Route Dart money parse/format through `minor_units` (**§3a** + report M5).
6. Add `cargo-audit` and a codegen-`--check` step to CI (**§6**, **§2c**).

**Larger, do together:**
7. Unify the app-lock + vault under one Argon2id KDF and give the lock cryptographic
   control of the db key, fixing the keystore-less plaintext-key gap (**§4a/b/c**).
8. Projection caching + single-fold queries (report **H2**) — the cost is amplified by the
   shell, which re-snapshots (groups + friends + activity + per-detail providers) on every
   single mutation, i.e. several full re-folds per user tap.
9. Adopt `tracing` at the use-case/trust boundaries ahead of the sync crate (**§5**).
