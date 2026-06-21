//! Shared test scaffolding: signed-op builders, a fresh replica, and an
//! in-memory [`Transport`] duplex so the async sync driver can be exercised
//! without a real network.
// Each integration-test binary pulls in this module but uses a different subset.
#![allow(dead_code)]

use splittr_crdt::{
    Cents, ExpenseFields, ExpenseId, GroupId, Hlc, Op, OpKind, SigningKey, SiteId, Split, UserId,
};
use splittr_store::{MemoryOpStore, Repository};
use splittr_sync::{SyncMessage, SyncSession, Transport};
use tokio::sync::mpsc;

pub type Replica = Repository<MemoryOpStore>;

/// A fresh, empty replica (in-memory op-log + fold).
pub fn replica() -> Replica {
    Repository::open(MemoryOpStore::new()).unwrap()
}

/// Drive the pure [`SyncSession`] protocol between two replicas to quiescence
/// (no transport, no async) — the deterministic core for example + property tests.
pub fn converge(a: &mut Replica, b: &mut Replica) {
    let (mut sa, mut sb) = (SyncSession::new(), SyncSession::new());
    let mut to_b = vec![sa.open(a)];
    let mut to_a: Vec<SyncMessage> = vec![];
    while !to_a.is_empty() || !to_b.is_empty() {
        // Replies B produces are bound for A, and vice versa.
        let mut from_b = vec![];
        for m in to_b.drain(..) {
            from_b.extend(sb.handle(m, b));
        }
        let mut from_a = vec![];
        for m in to_a.drain(..) {
            from_a.extend(sa.handle(m, a));
        }
        to_a = from_b;
        to_b = from_a;
    }
}

/// A fixed pool of valid, distinct ops from two founders plus a profile op — the
/// universe a partition test distributes across replicas.
pub fn op_pool() -> Vec<Op> {
    let mut pool = world(&key(1), "a");
    pool.extend(world(&key(2), "b"));
    pool.push(op(
        &key(1),
        20,
        OpKind::UpsertProfile {
            user: UserId::from("user:guest-a"),
            name: "Guest".into(),
        },
    ));
    pool
}

/// A deterministic signing key from a one-byte seed.
pub fn key(seed: u8) -> SigningKey {
    SigningKey::from_seed([seed; 32])
}

/// A signed op at a unique HLC (so distinct ops get distinct content ids).
pub fn op(k: &SigningKey, counter: u32, kind: OpKind) -> Op {
    let hlc = Hlc {
        wall_ms: 1_700_000_000_000 + counter as u64,
        counter,
        site: SiteId(1),
    };
    Op::signed(hlc, k, kind)
}

/// An authorized mini-world authored by `founder`: found a group and add two
/// equally-split expenses (founder pays, split with a placeholder). Returns the
/// founder's ops in order. The founder is auto-seeded as a member, so these
/// count toward balances (ADR-0006).
pub fn world(founder: &SigningKey, tag: &str) -> Vec<Op> {
    let g = GroupId::new(format!("trip-{tag}"));
    let guest = UserId::new(format!("user:guest-{tag}"));
    let me = splittr_crdt::user_id_for(&founder.public());
    let expense = |n: u32, cents: i64| {
        let fields = ExpenseFields::single_payer(
            me.clone(),
            Cents(cents),
            vec![
                Split {
                    user: me.clone(),
                    owed: Cents(cents / 2),
                },
                Split {
                    user: guest.clone(),
                    owed: Cents(cents / 2),
                },
            ],
        );
        OpKind::CreateExpense {
            expense: ExpenseId::new(format!("exp:{tag}:{n}")),
            group: Some(g.clone()),
            fields,
            draft: false,
        }
    };
    vec![
        op(
            founder,
            0,
            OpKind::CreateGroup {
                group: g.clone(),
                name: "Trip".into(),
            },
        ),
        op(founder, 1, expense(1, 3000)),
        op(founder, 2, expense(2, 1000)),
    ]
}

/// An in-memory [`Transport`]: an ordered, reliable duplex over tokio channels.
pub struct InMemTransport {
    tx: mpsc::Sender<SyncMessage>,
    rx: mpsc::Receiver<SyncMessage>,
}

impl Transport for InMemTransport {
    type Error = ();

    async fn send(&mut self, msg: SyncMessage) -> Result<(), ()> {
        self.tx.send(msg).await.map_err(|_| ())
    }

    async fn recv(&mut self) -> Result<SyncMessage, ()> {
        self.rx.recv().await.ok_or(())
    }
}

/// A connected pair of transports (each end sees the other's sends, in order).
pub fn duplex() -> (InMemTransport, InMemTransport) {
    let (a_tx, b_rx) = mpsc::channel(64);
    let (b_tx, a_rx) = mpsc::channel(64);
    (
        InMemTransport { tx: a_tx, rx: a_rx },
        InMemTransport { tx: b_tx, rx: b_rx },
    )
}
