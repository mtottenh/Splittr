//! Authorization derived purely from the op-set (#16/#38, ADR-0005/0006).
//!
//! Before the value fold ([`crate::materialize`]), [`Authority::build`] computes,
//! as an order-independent function of the whole op-set, the device→identity
//! certificates and revocations, the authorized alias map (#8), each group's
//! founder and time-sliced membership, confirmed friendships (#7), and the
//! group/parties each expense or settlement is bound to.
//!
//! [`Authority::entitled`] then decides whether a value op may fold: a member for
//! group ops, a confirmed friend (or a guest) for non-group ops, the subject for
//! profile/key ops, and a party for alias claims. This is the single home of the
//! "who may do what" policy, kept separate from the value fold.

use std::collections::{BTreeMap, BTreeSet};

use splittr_crypto::PublicKey;
use splittr_domain::{ExpenseFields, ExpenseId, GroupId, SettlementId, UserId};

use crate::clock::{Hlc, SiteId};
use crate::identity::{is_placeholder, user_id_for};
use crate::op::{Op, OpId, OpKind};
use crate::projection::DeviceRecord;

/// A deterministic total order over ops (HLC then content id), used to linearize
/// authorization decisions (membership, revocation) independently of arrival.
type Order = (Hlc, OpId);

/// Earlier than any real op — seeds a group founder as a member from creation.
fn min_order() -> Order {
    (
        Hlc {
            wall_ms: 0,
            counter: 0,
            site: SiteId(0),
        },
        OpId([0u8; 32]),
    )
}

fn order_of(op: &Op) -> Order {
    (op.hlc, op.id)
}

// ---------------------------------------------------------------------------
// Authority: who may do what, derived purely from the op-set (#16/#38).
// ---------------------------------------------------------------------------

/// Device certificates extracted from the op-set (#16/ADR-0005).
#[derive(Default)]
struct Devices {
    /// device pubkey → (hlc, identity, site) of the winning authorization.
    authorized: BTreeMap<PublicKey, (Hlc, PublicKey, u64)>,
    /// device pubkey → earliest revocation HLC.
    revoked: BTreeMap<PublicKey, Hlc>,
}

impl Devices {
    /// The identity a key acts for: its certificate's identity, or itself when
    /// uncertified (the self-sovereign default).
    fn identity_of(&self, author: &PublicKey) -> PublicKey {
        self.authorized
            .get(author)
            .map(|(_, identity, _)| *identity)
            .unwrap_or(*author)
    }

    /// Whether the op's author device is revoked as of the op's HLC.
    fn revoked_at(&self, op: &Op) -> bool {
        matches!(self.revoked.get(&op.author), Some(rh) if op.hlc >= *rh)
    }
}

/// Whether an `AddAlias{alias, canonical}` (#8) is authorized: the asserting
/// identity must be a party to the merge (claiming a guest as yourself, or
/// merging your own account), **or** both endpoints are placeholder guests
/// (anyone may reconcile two guests — low-stakes and reversible).
fn alias_authorized(raw_actor: &UserId, alias: &UserId, canonical: &UserId) -> bool {
    raw_actor == alias
        || raw_actor == canonical
        || (is_placeholder(alias) && is_placeholder(canonical))
}

/// First pass: build the device-certificate view. Authorize/revoke ops are
/// honoured only when self-signed by the identity (`op.author == identity`) —
/// root-only enrolment.
fn collect_devices<'a>(ops: impl Iterator<Item = &'a Op>) -> Devices {
    let mut d = Devices::default();
    for op in ops {
        match &op.kind {
            OpKind::AuthorizeDevice {
                identity,
                device,
                site,
            } if op.author == *identity => {
                let cand = (op.hlc, *identity, *site);
                match d.authorized.get(device) {
                    Some(existing) if *existing >= cand => {}
                    _ => {
                        d.authorized.insert(*device, cand);
                    }
                }
            }
            OpKind::RevokeDevice { identity, device } if op.author == *identity => {
                d.revoked
                    .entry(*device)
                    .and_modify(|h| {
                        if op.hlc < *h {
                            *h = op.hlc;
                        }
                    })
                    .or_insert(op.hlc);
            }
            _ => {}
        }
    }
    d
}

/// The whole-set authorization view: device certs, aliases, group founder +
/// time-sliced members, friendships, and the binding of each expense/settlement
/// to a group + parties. Built once per fold by [`Authority::build`].
pub(crate) struct Authority {
    devices: Devices,
    pub(crate) aliases: BTreeMap<UserId, UserId>,
    /// group → founding member (its creator): seeds membership and is the only
    /// actor whose `CreateGroup` (name) counts, so a forged re-create can't rename.
    founders: BTreeMap<GroupId, UserId>,
    /// group → user → time-ordered authorized membership events. A user is a
    /// member *as of* an order iff its latest event at-or-before is `true`, so a
    /// removal only affects later ops — a member's past activity is preserved.
    membership: BTreeMap<GroupId, BTreeMap<UserId, Vec<(Order, bool)>>>,
    expense_group: BTreeMap<ExpenseId, Option<GroupId>>,
    expense_parties: BTreeMap<ExpenseId, BTreeSet<UserId>>,
    settlement_group: BTreeMap<SettlementId, Option<GroupId>>,
    settlement_parties: BTreeMap<SettlementId, BTreeSet<UserId>>,
    /// declarer → set of users they declared friendship toward (#7). A mutual
    /// pair is a confirmed friend edge.
    friend_decls: BTreeMap<UserId, BTreeSet<UserId>>,
}

impl Authority {
    /// Resolve a user id through the alias map to its canonical id (#8).
    fn resolve(&self, user: &UserId) -> UserId {
        self.aliases
            .get(user)
            .cloned()
            .unwrap_or_else(|| user.clone())
    }

    /// The canonical user id acting in `op`: its author's identity (via cert,
    /// default self) mapped to a user id, then alias-resolved.
    fn actor(&self, op: &Op) -> UserId {
        self.resolve(&user_id_for(&self.devices.identity_of(&op.author)))
    }

    /// Whether `a` and `b` have a confirmed (mutual) friendship (#7).
    fn is_friend(&self, a: &UserId, b: &UserId) -> bool {
        self.friend_decls.get(a).is_some_and(|s| s.contains(b))
            && self.friend_decls.get(b).is_some_and(|s| s.contains(a))
    }

    /// Whether `actor` may act on a non-group (friend) op over `parties`: it must
    /// be a participant, and every *other real-identity* participant must be a
    /// confirmed friend (placeholder guests are exempt — they can't forge ops, so
    /// tracking a guest needs no reciprocation) (#38).
    fn friend_op_ok(&self, actor: &UserId, parties: &BTreeSet<UserId>) -> bool {
        parties.contains(actor)
            && parties
                .iter()
                .all(|p| p == actor || is_placeholder(p) || self.is_friend(actor, p))
    }

    /// The device records for the projection: every certified device with its
    /// identity, site, and whether it has been revoked (#16).
    pub(crate) fn device_records(&self) -> BTreeMap<PublicKey, DeviceRecord> {
        self.devices
            .authorized
            .iter()
            .map(|(device, (_hlc, identity, site))| {
                (
                    *device,
                    DeviceRecord {
                        identity: *identity,
                        site: *site,
                        revoked: self.devices.revoked.contains_key(device),
                    },
                )
            })
            .collect()
    }

    /// The confirmed (mutual) friend adjacency for the projection (#7).
    pub(crate) fn confirmed_friends(&self) -> BTreeMap<UserId, BTreeSet<UserId>> {
        let mut out: BTreeMap<UserId, BTreeSet<UserId>> = BTreeMap::new();
        for (a, declared) in &self.friend_decls {
            for b in declared {
                if self.is_friend(a, b) {
                    out.entry(a.clone()).or_default().insert(b.clone());
                }
            }
        }
        out
    }

    /// Whether `user` is a member of `group` as of order `at` (the latest
    /// membership event at-or-before `at` is `true`).
    fn is_member_as_of(&self, group: &GroupId, user: &UserId, at: Order) -> bool {
        self.membership
            .get(group)
            .and_then(|m| m.get(user))
            .and_then(|events| events.iter().rev().find(|(o, _)| *o <= at).map(|(_, m)| *m))
            .unwrap_or(false)
    }

    /// The group's *current* members (latest event per user is `true`) — for the
    /// projection's member list.
    pub(crate) fn current_members(&self, group: &GroupId) -> BTreeSet<UserId> {
        self.membership
            .get(group)
            .map(|m| {
                m.iter()
                    .filter(|(_, ev)| ev.last().is_some_and(|(_, is)| *is))
                    .map(|(u, _)| u.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Participants of an expense version (payers ∪ split users), alias-resolved.
    fn parties_of(&self, fields: &ExpenseFields) -> BTreeSet<UserId> {
        let mut set = BTreeSet::new();
        for u in fields.paid_by.keys() {
            set.insert(self.resolve(u));
        }
        for s in &fields.splits {
            set.insert(self.resolve(&s.user));
        }
        set
    }

    pub(crate) fn build<'a>(ops: impl Iterator<Item = &'a Op> + Clone) -> Authority {
        let devices = collect_devices(ops.clone());

        // Stage 1 — aliases. Authorize an AddAlias by its author's *raw* identity
        // (no alias resolution yet, to avoid circularity): you may only assert a
        // merge you are party to. Then resolve to canonical ids (#8).
        let mut edges = Vec::new();
        for op in ops.clone() {
            if let OpKind::AddAlias { alias, canonical } = &op.kind {
                if devices.revoked_at(op) {
                    continue;
                }
                let raw = user_id_for(&devices.identity_of(&op.author));
                if alias_authorized(&raw, alias, canonical) {
                    edges.push((alias.clone(), canonical.clone()));
                }
            }
        }
        let aliases = resolve_aliases(&edges);

        let mut auth = Authority {
            devices,
            aliases,
            founders: BTreeMap::new(),
            membership: BTreeMap::new(),
            expense_group: BTreeMap::new(),
            expense_parties: BTreeMap::new(),
            settlement_group: BTreeMap::new(),
            settlement_parties: BTreeMap::new(),
            friend_decls: BTreeMap::new(),
        };

        // Stage 1.5 — friendships. Each DeclareFriend is a self-signed intent
        // (no entitlement gate); a mutual pair becomes a confirmed edge (#7).
        for op in ops.clone() {
            if let OpKind::DeclareFriend { other } = &op.kind {
                if auth.devices.revoked_at(op) {
                    continue;
                }
                let (actor, other) = (auth.actor(op), auth.resolve(other));
                if actor != other {
                    auth.friend_decls.entry(actor).or_default().insert(other);
                }
            }
        }

        // Stage 2 — founders. A group's founder is the actor of its *earliest*
        // CreateGroup (by order), so a later forged re-create can't seize it. The
        // founder is seeded as a member from the start of time.
        let mut founder_pick: BTreeMap<GroupId, (Order, UserId)> = BTreeMap::new();
        for op in ops.clone() {
            if let OpKind::CreateGroup { group, .. } = &op.kind {
                if auth.devices.revoked_at(op) {
                    continue;
                }
                let key = order_of(op);
                let actor = auth.actor(op);
                match founder_pick.get(group) {
                    Some((existing, _)) if *existing <= key => {}
                    _ => {
                        founder_pick.insert(group.clone(), (key, actor));
                    }
                }
            }
        }
        for (group, (_, founder)) in founder_pick {
            auth.membership
                .entry(group.clone())
                .or_default()
                .entry(founder.clone())
                .or_default()
                .push((min_order(), true));
            auth.founders.insert(group, founder);
        }

        // Stage 3 — membership. Replay authorized SetMembership in order (a
        // deterministic linearization, so order-independent). Any member *as of*
        // the op may add or remove members (a communal model for v1); each
        // authorized change is recorded as a timeline event.
        let mut membership: Vec<&Op> = ops
            .clone()
            .filter(|op| matches!(op.kind, OpKind::SetMembership { .. }))
            .collect();
        membership.sort_by_key(|op| order_of(op));
        for op in membership {
            if auth.devices.revoked_at(op) {
                continue;
            }
            if let OpKind::SetMembership {
                group,
                user,
                member,
            } = &op.kind
            {
                let actor = auth.actor(op);
                let at = order_of(op);
                if auth.is_member_as_of(group, &actor, at) {
                    let user = auth.resolve(user);
                    auth.membership
                        .entry(group.clone())
                        .or_default()
                        .entry(user)
                        .or_default()
                        .push((at, *member));
                }
            }
        }

        // Stage 4 — entity bindings. Record the group + parties of each authorized
        // create op (LWW by HLC), so dependent ops (edit/void/publish/lock) can be
        // authorized against the same context.
        let mut expense_at: BTreeMap<ExpenseId, Hlc> = BTreeMap::new();
        let mut settlement_at: BTreeMap<SettlementId, Hlc> = BTreeMap::new();
        for op in ops.clone() {
            if auth.devices.revoked_at(op) {
                continue;
            }
            let actor = auth.actor(op);
            let at = order_of(op);
            match &op.kind {
                OpKind::CreateExpense {
                    expense,
                    group,
                    fields,
                    ..
                } => {
                    let parties = auth.parties_of(fields);
                    let ok = match group {
                        Some(g) => auth.is_member_as_of(g, &actor, at),
                        None => auth.friend_op_ok(&actor, &parties),
                    };
                    if ok && expense_at.get(expense).is_none_or(|h| op.hlc > *h) {
                        expense_at.insert(expense.clone(), op.hlc);
                        auth.expense_group.insert(expense.clone(), group.clone());
                        auth.expense_parties.insert(expense.clone(), parties);
                    }
                }
                OpKind::RecordSettlement {
                    settlement,
                    group,
                    from,
                    to,
                    ..
                } => {
                    let (from, to) = (auth.resolve(from), auth.resolve(to));
                    let parties: BTreeSet<UserId> = [from.clone(), to.clone()].into();
                    let ok = match group {
                        Some(g) => auth.is_member_as_of(g, &actor, at),
                        None => auth.friend_op_ok(&actor, &parties),
                    };
                    if ok && settlement_at.get(settlement).is_none_or(|h| op.hlc > *h) {
                        settlement_at.insert(settlement.clone(), op.hlc);
                        auth.settlement_group
                            .insert(settlement.clone(), group.clone());
                        auth.settlement_parties.insert(settlement.clone(), parties);
                    }
                }
                _ => {}
            }
        }

        auth
    }

    /// Whether a value op is entitled to fold. Authenticity is already enforced
    /// at ingestion (`Op::verify`); this is the *authorization* layer.
    pub(crate) fn entitled(&self, op: &Op) -> bool {
        if self.devices.revoked_at(op) {
            return false;
        }
        let actor = self.actor(op);
        let at = order_of(op);
        match &op.kind {
            // Only the founder's CreateGroup counts (creates the group + name).
            OpKind::CreateGroup { group, .. } => self.founders.get(group) == Some(&actor),
            // Group settings + freezing the books: any current member.
            OpKind::SetGroupName { group, .. }
            | OpKind::SetGroupCurrency { group, .. }
            | OpKind::SetClosedPeriod { group, .. } => self.is_member_as_of(group, &actor, at),
            // Membership is owned by the Authority pass, not the value fold.
            OpKind::SetMembership { .. } => false,
            OpKind::CreateExpense { group, fields, .. } => match group {
                Some(g) => self.is_member_as_of(g, &actor, at),
                None => self.friend_op_ok(&actor, &self.parties_of(fields)),
            },
            OpKind::EditExpense { expense, .. }
            | OpKind::PublishExpense { expense }
            | OpKind::VoidExpense { expense }
            | OpKind::SetExpenseLock { expense, .. } => self.expense_entitled(expense, &actor, at),
            OpKind::RecordSettlement {
                group, from, to, ..
            } => match group {
                Some(g) => self.is_member_as_of(g, &actor, at),
                None => {
                    let parties = [self.resolve(from), self.resolve(to)].into();
                    self.friend_op_ok(&actor, &parties)
                }
            },
            OpKind::VoidSettlement { settlement } => {
                self.settlement_entitled(settlement, &actor, at)
            }
            // Friendship declarations are owned by the Authority pass.
            OpKind::DeclareFriend { .. } => false,
            // Aliases are owned by the Authority pass; re-check the same rule.
            OpKind::AddAlias { alias, canonical } => {
                let raw = user_id_for(&self.devices.identity_of(&op.author));
                alias_authorized(&raw, alias, canonical)
            }
            // You may set your own profile, or name an unclaimed placeholder.
            OpKind::UpsertProfile { user, .. } => {
                let user = self.resolve(user);
                actor == user || is_placeholder(&user)
            }
            OpKind::SetAgreementKey { user, .. } => actor == self.resolve(user),
            // Certs are consumed in `collect_devices`, never value-folded.
            OpKind::AuthorizeDevice { .. } | OpKind::RevokeDevice { .. } => false,
        }
    }

    fn expense_entitled(&self, expense: &ExpenseId, actor: &UserId, at: Order) -> bool {
        match self.expense_group.get(expense) {
            Some(Some(group)) => self.is_member_as_of(group, actor, at),
            Some(None) => self
                .expense_parties
                .get(expense)
                .is_some_and(|p| self.friend_op_ok(actor, p)),
            None => false, // no authorized creating op → nothing to act on
        }
    }

    fn settlement_entitled(&self, settlement: &SettlementId, actor: &UserId, at: Order) -> bool {
        match self.settlement_group.get(settlement) {
            Some(Some(group)) => self.is_member_as_of(group, actor, at),
            Some(None) => self
                .settlement_parties
                .get(settlement)
                .is_some_and(|p| self.friend_op_ok(actor, p)),
            None => false,
        }
    }
}

fn resolve_aliases(edges: &[(UserId, UserId)]) -> BTreeMap<UserId, UserId> {
    let mut adjacency: BTreeMap<UserId, BTreeSet<UserId>> = BTreeMap::new();
    let mut nodes: BTreeSet<UserId> = BTreeSet::new();
    for (a, b) in edges {
        nodes.insert(a.clone());
        nodes.insert(b.clone());
        adjacency.entry(a.clone()).or_default().insert(b.clone());
        adjacency.entry(b.clone()).or_default().insert(a.clone());
    }

    let mut canonical: BTreeMap<UserId, UserId> = BTreeMap::new();
    let mut visited: BTreeSet<UserId> = BTreeSet::new();
    for start in &nodes {
        if visited.contains(start) {
            continue;
        }
        let mut component: Vec<UserId> = Vec::new();
        let mut stack = vec![start.clone()];
        visited.insert(start.clone());
        while let Some(u) = stack.pop() {
            component.push(u.clone());
            if let Some(neighbours) = adjacency.get(&u) {
                for v in neighbours {
                    if visited.insert(v.clone()) {
                        stack.push(v.clone());
                    }
                }
            }
        }
        let canon = component
            .iter()
            .min()
            .cloned()
            .expect("non-empty component");
        for u in component {
            // A node mapped to itself isn't an alias; only record real redirects.
            if u != canon {
                canonical.insert(u, canon.clone());
            }
        }
    }
    canonical
}
