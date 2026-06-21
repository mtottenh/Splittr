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
///
/// **Wire-format contract — append-only.** These bytes are content-addressed,
/// signed (`canonical`), and persisted via **postcard**, a non-self-describing
/// format that encodes each enum variant by its *ordinal index* and each struct
/// field by *declaration order* (no names, no tags). So this enum is an
/// on-disk/on-wire API, not just a type:
/// - **Never reorder or remove** a variant, and only ever **append** a new one at
///   the end. Reordering renumbers later variants, so existing stored/synced ops
///   silently decode as the wrong variant — and, because the bytes feed the
///   `OpId` and signature, the *same* logical op would get a different id and a
///   non-verifying signature, breaking dedup and convergence.
/// - **Never add, remove, or reorder the fields** of an existing variant, for the
///   same reason. New capability = a **new variant appended here**, never a change
///   to an existing one (the event-sourcing-friendly path: old ops stay valid
///   forever).
///
/// The `op_kind_discriminants_are_frozen` / `op_canonical_bytes_are_frozen`
/// golden tests pin this; if one fails you changed the format — append a new
/// variant instead, and treat any deliberate format change as a versioned
/// migration.
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
    /// Declare a one-way friendship intent toward `other` (#7). Self-signed;
    /// a **mutual** declaration (both parties) forms a friend edge that gates
    /// non-group friend expenses (#38). Grow-only and order-independent.
    DeclareFriend {
        other: UserId,
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
    /// Authorize a device key to act for an identity (#16, ADR-0005). Valid only
    /// when signed by `identity` (`op.author == identity`) — root-only enrolment.
    /// Establishes `device → identity`; normal ops are then signed by `device`.
    AuthorizeDevice {
        identity: PublicKey,
        device: PublicKey,
        site: u64,
    },
    /// Revoke a previously-authorized device (#16). Terminal; signed by the
    /// identity. Ops authored by `device` at/after this op's HLC stop counting.
    RevokeDevice {
        identity: PublicKey,
        device: PublicKey,
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

    /// Verify the content hash and the author's signature at the trust boundary.
    /// `Ok(())` for a genuine op; an [`VerifyError`] distinguishes corruption (the
    /// id doesn't match the bytes) from forgery (a bad signature) so callers can
    /// log the cause — useful once ops arrive over the network (#14/#20).
    pub fn verify(&self) -> Result<(), VerifyError> {
        let content = canonical(&self.hlc, &self.author, &self.kind);
        if *blake3::hash(&content).as_bytes() != self.id.0 {
            return Err(VerifyError::HashMismatch);
        }
        if !verify(&self.author, &content, &self.sig) {
            return Err(VerifyError::BadSignature);
        }
        Ok(())
    }
}

/// Why an [`Op`] failed verification (the trust-boundary failure reason).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerifyError {
    /// The content hash doesn't match the id — corruption or tampering.
    HashMismatch,
    /// The signature doesn't verify against the author — forgery.
    BadSignature,
}

impl core::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VerifyError::HashMismatch => f.write_str("op content hash does not match its id"),
            VerifyError::BadSignature => f.write_str("op signature does not verify"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// The canonical byte encoding of an op's content — the input to both the id
/// hash and the signature (the single definition of "the bytes that matter").
fn canonical(hlc: &Hlc, author: &PublicKey, kind: &OpKind) -> Vec<u8> {
    postcard::to_allocvec(&(hlc, author, kind)).expect("canonical encoding is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SiteId;
    use splittr_domain::ExpenseFields;
    use std::collections::BTreeMap;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// One representative instance of every `OpKind`, in declaration order. The
    /// position in this list is the postcard discriminant the wire format pins.
    fn one_of_each() -> Vec<OpKind> {
        let g = || GroupId::from("g");
        let u = || UserId::from("u");
        let e = || ExpenseId::from("e");
        let s = || SettlementId::from("s");
        let fields = || ExpenseFields::new(BTreeMap::new(), Cents(0), vec![]);
        let pk = || PublicKey([0u8; 32]);
        vec![
            OpKind::CreateGroup {
                group: g(),
                name: "n".into(),
            },
            OpKind::SetGroupName {
                group: g(),
                name: "n".into(),
            },
            OpKind::SetGroupCurrency {
                group: g(),
                currency: "USD".into(),
            },
            OpKind::SetMembership {
                group: g(),
                user: u(),
                member: true,
            },
            OpKind::CreateExpense {
                expense: e(),
                group: Some(g()),
                fields: fields(),
                draft: false,
            },
            OpKind::EditExpense {
                expense: e(),
                fields: fields(),
            },
            OpKind::PublishExpense { expense: e() },
            OpKind::VoidExpense { expense: e() },
            OpKind::SetExpenseLock {
                expense: e(),
                locked: true,
            },
            OpKind::SetClosedPeriod {
                group: g(),
                until_ms: 0,
            },
            OpKind::RecordSettlement {
                settlement: s(),
                group: Some(g()),
                from: u(),
                to: u(),
                amount: Cents(0),
            },
            OpKind::VoidSettlement { settlement: s() },
            OpKind::AddAlias {
                alias: u(),
                canonical: u(),
            },
            OpKind::DeclareFriend { other: u() },
            OpKind::UpsertProfile {
                user: u(),
                name: "n".into(),
            },
            OpKind::SetAgreementKey {
                user: u(),
                key: [0u8; 32],
            },
            OpKind::AuthorizeDevice {
                identity: pk(),
                device: pk(),
                site: 0,
            },
            OpKind::RevokeDevice {
                identity: pk(),
                device: pk(),
            },
        ]
    }

    #[test]
    fn op_kind_discriminants_are_frozen() {
        // postcard encodes the enum variant as a leading varint = declaration
        // index (a single byte for 0..=17). If a variant is reordered/removed,
        // these shift and previously-stored ops mis-decode — see the OpKind
        // wire-format contract. New capability must be *appended*, not inserted.
        let kinds = one_of_each();
        assert_eq!(kinds.len(), 18, "every OpKind variant is represented");
        for (want, kind) in kinds.iter().enumerate() {
            let bytes = postcard::to_allocvec(kind).unwrap();
            assert_eq!(bytes[0] as usize, want, "discriminant for {kind:?}");
        }
    }

    #[test]
    fn op_canonical_bytes_are_frozen() {
        // A fixed op → fixed canonical bytes → fixed id and signature. If this
        // golden hex changes, the wire format changed (id/signature-breaking).
        // Only update it as a deliberate, versioned migration.
        let key = SigningKey::from_seed([0x42; 32]);
        let hlc = Hlc {
            wall_ms: 1_700_000_000_000,
            counter: 7,
            site: SiteId(3),
        };
        let kind = OpKind::CreateGroup {
            group: GroupId::from("g0"),
            name: "Trip".into(),
        };
        let content = canonical(&hlc, &key.public(), &kind);
        assert_eq!(
            hex(&content),
            "80d095ffbc3107032152f8d19b791d24453242e15f2eab6cb7cffa7b6a5ed30097960e069881db12000267300454726970"
        );
        // The id is the BLAKE3 of those bytes; pin it too.
        let op = Op::signed(hlc, &key, kind);
        assert_eq!(
            hex(&op.id.0),
            "c458f448d8c71a1293e629e52a3856b75c6feecc6dabc83e9ac023840099fe64"
        );
    }
}
