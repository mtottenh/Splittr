//! Operations — the immutable, content-addressed, signed units of the log.

use serde::{Deserialize, Serialize};
use splittr_crypto::{sign, verify, PublicKey, Signature, SigningKey};
use splittr_domain::{Cents, ExpenseFields, ExpenseId, GroupId, SettlementId, UserId};

use crate::clock::Hlc;

/// Content-addressed operation id: BLAKE3 over the canonical encoding of the
/// op's content `(hlc, author, kind)`. Equal content ⇒ equal id ⇒ free
/// deduplication; tamper-evident.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct OpId(pub [u8; 32]);

/// The operation vocabulary (ADR-0001 §Operation vocabulary). A focused subset
/// that exercises every conflict rule; lock/identity ops join with #15/#6/#16.
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
    /// The group's base/display currency — LWW (rule 2). Default `USD` (#3).
    SetGroupCurrency {
        group: GroupId,
        currency: String,
    },
    /// Membership register — LWW per `(group, user)` (rule 3).
    SetMembership {
        group: GroupId,
        user: UserId,
        member: bool,
    },
    CreateExpense {
        expense: ExpenseId,
        /// `None` for a non-group (friend-to-friend) expense (#31).
        group: Option<GroupId>,
        fields: ExpenseFields,
        /// Created as a private draft — excluded from balances until published.
        draft: bool,
    },
    /// A new whole version of an expense — whole-version LWW (rule 4).
    EditExpense {
        expense: ExpenseId,
        fields: ExpenseFields,
    },
    /// Publish a draft expense so it counts toward balances. Monotonic
    /// (publish-wins), like a positive tombstone — order-independent.
    PublishExpense {
        expense: ExpenseId,
    },
    /// Terminal tombstone — delete wins (rule 5).
    VoidExpense {
        expense: ExpenseId,
    },
    /// Lock/unlock an expense — LWW register (rule 6). Lock is an authorization
    /// guard (enforced at command time), not a conflict axis: it never changes
    /// version/void resolution.
    SetExpenseLock {
        expense: ExpenseId,
        locked: bool,
    },
    /// Close a group's accounting period: expenses dated at or before `until_ms`
    /// can no longer be edited/deleted. LWW register — lowering it is a
    /// privileged reopen.
    SetClosedPeriod {
        group: GroupId,
        until_ms: i64,
    },
    RecordSettlement {
        settlement: SettlementId,
        /// `None` for a non-group (friend-to-friend) settlement (#31).
        group: Option<GroupId>,
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
    /// A user's display profile — LWW by HLC (rule 2). Covers identity/people
    /// (#6/#2): a real user or a placeholder, both just have a name here.
    UpsertProfile {
        user: UserId,
        name: String,
    },
    /// Publish a user's X25519 agreement public key — LWW (rule 2). Lets peers
    /// derive a shared secret to encrypt content to them (#6, consumed by #14).
    SetAgreementKey {
        user: UserId,
        key: [u8; 32],
    },
}

/// An immutable, content-addressed, signed operation.
///
/// `author` is the signer's public key and `sig` is its Ed25519 signature over
/// the canonical content. [`Op::verify`] checks both the content hash and the
/// signature; call it at trust boundaries (op ingestion). The pure fold
/// ([`crate::Materializer`]) trusts the ops it is handed — ingestion is where
/// authenticity is enforced (ADR-0001).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Op {
    pub id: OpId,
    pub hlc: Hlc,
    pub author: PublicKey,
    pub sig: Signature,
    pub kind: OpKind,
}

impl Op {
    /// Build and sign an op with `key`.
    pub fn signed(hlc: Hlc, key: &SigningKey, kind: OpKind) -> Self {
        let author = key.public();
        let content = canonical(&hlc, &author, &kind);
        let id = OpId(*blake3::hash(&content).as_bytes());
        let sig = sign(key, &content);
        Op {
            id,
            hlc,
            author,
            sig,
            kind,
        }
    }

    /// Verify the content hash and the author's signature. Returns `false` for a
    /// forged, corrupted, or tampered op.
    pub fn verify(&self) -> bool {
        let content = canonical(&self.hlc, &self.author, &self.kind);
        *blake3::hash(&content).as_bytes() == self.id.0 && verify(&self.author, &content, &self.sig)
    }
}

/// The canonical byte encoding of an op's content — the input to both the id
/// hash and the signature (the single definition of "the bytes that matter").
fn canonical(hlc: &Hlc, author: &PublicKey, kind: &OpKind) -> Vec<u8> {
    postcard::to_allocvec(&(hlc, author, kind)).expect("canonical encoding is infallible")
}
