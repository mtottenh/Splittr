//! Operations — the immutable, content-addressed units of the log.

use serde::{Deserialize, Serialize};
use splittr_domain::{Cents, ExpenseId, GroupId, SettlementId, Split, UserId};

use crate::clock::Hlc;

/// The identity that authored an op. A placeholder until real keypairs (#6).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ActorId(pub String);

/// Content-addressed operation id: BLAKE3 over the canonical encoding of
/// `(hlc, author, kind)`. Equal content ⇒ equal id ⇒ free deduplication.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct OpId(pub [u8; 32]);

/// The operation vocabulary (ADR-0001 §Operation vocabulary). A focused subset
/// that exercises every conflict rule; identity/key ops join with #6/#14/#16.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OpKind {
    CreateGroup {
        group: GroupId,
        name: String,
    },
    /// Mutable scalar — LWW (rule 2). Representative of the eventual `SetGroupMeta`.
    SetGroupName {
        group: GroupId,
        name: String,
    },
    /// Membership register — LWW per `(group, user)` (rule 3).
    SetMembership {
        group: GroupId,
        user: UserId,
        member: bool,
    },
    CreateExpense {
        expense: ExpenseId,
        group: GroupId,
        payer: UserId,
        total: Cents,
        splits: Vec<Split>,
    },
    /// A new whole version of an expense — whole-version LWW (rule 4).
    EditExpense {
        expense: ExpenseId,
        payer: UserId,
        total: Cents,
        splits: Vec<Split>,
    },
    /// Terminal tombstone — delete wins (rule 5).
    VoidExpense {
        expense: ExpenseId,
    },
    RecordSettlement {
        settlement: SettlementId,
        group: GroupId,
        from: UserId,
        to: UserId,
        amount: Cents,
    },
    VoidSettlement {
        settlement: SettlementId,
    },
    /// Claim/merge edge (#8) — union-find to a canonical id (rule 6).
    AddAlias {
        alias: UserId,
        canonical: UserId,
    },
}

/// An immutable, content-addressed operation.
///
/// Signing/verification (ADR-0001 §operation model) lands with #6/#14/#16; for
/// now [`Op::author`] is an abstract actor and signatures are out of scope.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Op {
    pub id: OpId,
    pub hlc: Hlc,
    pub author: ActorId,
    pub kind: OpKind,
}

impl Op {
    /// Build an op, computing its content-addressed id from `(hlc, author, kind)`.
    pub fn new(hlc: Hlc, author: ActorId, kind: OpKind) -> Self {
        let id = content_id(&hlc, &author, &kind);
        Op {
            id,
            hlc,
            author,
            kind,
        }
    }
}

fn content_id(hlc: &Hlc, author: &ActorId, kind: &OpKind) -> OpId {
    let bytes = postcard::to_allocvec(&(hlc, author, kind))
        .expect("canonical encoding of an op is infallible");
    OpId(*blake3::hash(&bytes).as_bytes())
}
