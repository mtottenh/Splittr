//! The FFI API surface scanned by flutter_rust_bridge.
//!
//! [`Engine`] is an opaque handle Dart holds: it owns the local [`App`] behind a
//! `Mutex` (so the handle is shared and `Sync`), and every method locks, calls
//! the app, and marshals results to DTOs. Errors surface as `anyhow::Error`,
//! which FRB maps to a Dart exception.

use std::sync::Mutex;

use anyhow::{anyhow, Result};
use splittr_app::{
    user_id_for, App, Cents, ExpenseDraft, GroupId, Identity, Invite, PairingTranscript, PublicKey,
    RedbOpStore, SettlementId, SiteId, UserId,
};

use crate::convert::to_fields;
use crate::dto::{
    ActivityEntryDto, DeviceViewDto, ExpenseInput, FriendBalanceDto, FriendDetailDto,
    GroupDetailDto, GroupSummaryDto, InviteDto,
};

pub struct Engine {
    inner: Mutex<App<RedbOpStore>>,
}

impl Engine {
    /// Open (or create) the engine over a redb database at `db_path`.
    ///
    /// `identity_seed` (32 bytes) is the local user's signing key and `db_key`
    /// (32 bytes) encrypts the op-log at rest (#22). The platform supplies both
    /// from secure storage / a keystore (#6/#16) — they are never derived from,
    /// or stored next to, the database.
    pub fn open(
        db_path: String,
        identity_seed: Vec<u8>,
        device_seed: Vec<u8>,
        db_key: Vec<u8>,
        site: u64,
    ) -> Result<Engine> {
        let id_seed = seed32(&identity_seed, "identity seed")?;
        let dev_seed = seed32(&device_seed, "device seed")?;
        let identity = Identity::from_seeds(id_seed, dev_seed);
        Self::open_with(db_path, identity, db_key, site)
    }

    /// Open for daily use with the root **locked** (#34): the device key signs
    /// ops, the public identity is known, and privileged actions (enrol/revoke)
    /// require [`unlock_root`](Engine::unlock_root). The device must already be
    /// enrolled — i.e. this identity ran a full `open` on this device before.
    /// `identity_public` is the 32-byte identity (root) public key.
    pub fn open_device_only(
        db_path: String,
        identity_public: Vec<u8>,
        device_seed: Vec<u8>,
        db_key: Vec<u8>,
        site: u64,
    ) -> Result<Engine> {
        let id_pub = PublicKey(seed32(&identity_public, "identity public key")?);
        let dev_seed = seed32(&device_seed, "device seed")?;
        let identity = Identity::device_only(id_pub, dev_seed);
        Self::open_with(db_path, identity, db_key, site)
    }

    fn open_with(
        db_path: String,
        identity: Identity,
        db_key: Vec<u8>,
        site: u64,
    ) -> Result<Engine> {
        let key = seed32(&db_key, "db key")?;
        let store = RedbOpStore::open_encrypted(&db_path, key)?;
        let app = App::new(identity, store, SiteId(site))?;
        Ok(Engine {
            inner: Mutex::new(app),
        })
    }

    /// The identity (root) public key as hex — persist it after the first
    /// (full) open so later launches can `open_device_only` (#34).
    pub fn identity_public(&self) -> String {
        hex32(&self.lock().identity_public())
    }

    /// Unlock the root from its seed (after the app lock decrypts the vault) so
    /// device enrol/revoke can be signed; errors if the seed is for another
    /// identity (#34).
    pub fn unlock_root(&self, identity_seed: Vec<u8>) -> Result<()> {
        let seed = seed32(&identity_seed, "identity seed")?;
        self.lock().unlock_root(seed)?;
        Ok(())
    }

    /// Re-seal the root after a privileged action.
    pub fn lock_root(&self) {
        self.lock().lock_root();
    }

    pub fn is_root_unlocked(&self) -> bool {
        self.lock().root_unlocked()
    }

    // --- profiles & people -------------------------------------------------

    pub fn my_user_id(&self) -> String {
        self.lock().me().to_string()
    }

    pub fn my_name(&self) -> Option<String> {
        self.lock().my_name()
    }

    /// The local user's X25519 agreement public key as hex (#6/#14). `None`
    /// before a profile has published it (locked, never-named identity).
    pub fn my_agreement_public(&self) -> Option<String> {
        self.lock().my_agreement_public().map(|k| hex32(&k))
    }

    pub fn set_my_name(&self, name: String) -> Result<()> {
        self.lock().set_my_name(&name)?;
        Ok(())
    }

    pub fn add_person(&self, name: String) -> Result<String> {
        Ok(self.lock().add_person(&name)?.0)
    }

    /// Claim a placeholder person as the local identity (#8): their history
    /// resolves to you with no balance change.
    pub fn claim_person(&self, placeholder: String) -> Result<()> {
        self.lock().claim_person(&UserId::new(placeholder))?;
        Ok(())
    }

    /// Merge two ids that are the same person (#8), keeping `keep` as canonical.
    pub fn merge_people(&self, duplicate: String, keep: String) -> Result<()> {
        self.lock()
            .merge_people(&UserId::new(duplicate), &UserId::new(keep))?;
        Ok(())
    }

    // --- friends & invites (#7) -------------------------------------------

    /// Declare friendship toward another identity (#7). Confirmed once mutual.
    pub fn add_friend(&self, user_id: String) -> Result<()> {
        self.lock().add_friend(&UserId::new(user_id))?;
        Ok(())
    }

    /// The local user's confirmed (mutual) friends (#7).
    pub fn confirmed_friends(&self) -> Vec<String> {
        self.lock()
            .confirmed_friends()
            .into_iter()
            .map(|u| u.0)
            .collect()
    }

    /// A signed friend invite for the local identity, valid until `expiry_ms`
    /// (Unix ms). Encodes to an opaque blob for a link/QR (#7). Needs the root
    /// unlocked (#34).
    pub fn create_friend_invite(&self, expiry_ms: u64) -> Result<Vec<u8>> {
        Ok(self
            .lock()
            .create_invite("friend".into(), expiry_ms)?
            .to_bytes())
    }

    /// A signed invite to join `group_id`, valid until `expiry_ms` (#7).
    pub fn create_group_invite(&self, group_id: String, expiry_ms: u64) -> Result<Vec<u8>> {
        Ok(self
            .lock()
            .create_invite(format!("group:{group_id}"), expiry_ms)?
            .to_bytes())
    }

    // --- groups & members --------------------------------------------------

    pub fn create_group(
        &self,
        name: String,
        member_ids: Vec<String>,
        currency: String,
    ) -> Result<String> {
        let members: Vec<UserId> = member_ids.into_iter().map(UserId::new).collect();
        let mut app = self.lock();
        let group = app.create_group(&name, &members)?;
        if currency != "USD" {
            app.set_group_currency(&group, &currency)?;
        }
        Ok(group.0)
    }

    pub fn rename_group(&self, group_id: String, name: String) -> Result<()> {
        self.lock().rename_group(&GroupId::new(group_id), &name)?;
        Ok(())
    }

    pub fn set_group_currency(&self, group_id: String, currency: String) -> Result<()> {
        self.lock()
            .set_group_currency(&GroupId::new(group_id), &currency)?;
        Ok(())
    }

    pub fn add_member(&self, group_id: String, user_id: String) -> Result<()> {
        self.lock()
            .add_member(&GroupId::new(group_id), &UserId::new(user_id))?;
        Ok(())
    }

    pub fn remove_member(&self, group_id: String, user_id: String) -> Result<()> {
        self.lock()
            .remove_member(&GroupId::new(group_id), &UserId::new(user_id))?;
        Ok(())
    }

    // --- expenses ----------------------------------------------------------

    pub fn add_expense(&self, input: ExpenseInput) -> Result<String> {
        let draft = ExpenseDraft {
            group: input.group_id.clone().map(GroupId::new),
            draft: input.draft,
            fields: to_fields(input),
        };
        Ok(self.lock().add_expense(draft)?.0)
    }

    /// Publish a draft expense so it counts toward balances (#15).
    pub fn publish_expense(&self, expense_id: String) -> Result<()> {
        self.lock()
            .publish_expense(&splittr_app::ExpenseId::new(expense_id))?;
        Ok(())
    }

    /// Close a group's accounting period (#15): expenses dated at or before
    /// `until_ms` become uneditable. Pass `0` to reopen.
    pub fn set_closed_period(&self, group_id: String, until_ms: i64) -> Result<()> {
        self.lock()
            .set_closed_period(&GroupId::new(group_id), until_ms)?;
        Ok(())
    }

    /// Replace an expense with a new version. `input.group_id`/`draft` are
    /// ignored (an expense cannot change groups, and edit preserves draft state).
    pub fn edit_expense(&self, expense_id: String, input: ExpenseInput) -> Result<()> {
        self.lock()
            .edit_expense(&splittr_app::ExpenseId::new(expense_id), to_fields(input))?;
        Ok(())
    }

    pub fn delete_expense(&self, expense_id: String) -> Result<()> {
        self.lock()
            .delete_expense(&splittr_app::ExpenseId::new(expense_id))?;
        Ok(())
    }

    pub fn lock_expense(&self, expense_id: String) -> Result<()> {
        self.lock()
            .lock_expense(&splittr_app::ExpenseId::new(expense_id))?;
        Ok(())
    }

    pub fn unlock_expense(&self, expense_id: String) -> Result<()> {
        self.lock()
            .unlock_expense(&splittr_app::ExpenseId::new(expense_id))?;
        Ok(())
    }

    // --- settlements -------------------------------------------------------

    pub fn record_settlement(
        &self,
        group_id: String,
        from: String,
        to: String,
        amount_cents: i64,
    ) -> Result<String> {
        Ok(self
            .lock()
            .record_settlement(
                &GroupId::new(group_id),
                &UserId::new(from),
                &UserId::new(to),
                Cents(amount_cents),
            )?
            .0)
    }

    /// Record a non-group (friend-to-friend) payment (#31).
    pub fn record_non_group_settlement(
        &self,
        from: String,
        to: String,
        amount_cents: i64,
    ) -> Result<String> {
        Ok(self
            .lock()
            .record_non_group_settlement(&UserId::new(from), &UserId::new(to), Cents(amount_cents))?
            .0)
    }

    pub fn delete_settlement(&self, settlement_id: String) -> Result<()> {
        self.lock()
            .delete_settlement(&SettlementId::new(settlement_id))?;
        Ok(())
    }

    // --- queries -----------------------------------------------------------

    pub fn groups(&self) -> Vec<GroupSummaryDto> {
        self.lock().groups().into_iter().map(Into::into).collect()
    }

    /// The local user's overall net across all expenses/settlements, including
    /// non-group ones (#31). Positive = owed to you.
    pub fn overall_net_cents(&self) -> i64 {
        self.lock().overall_net().0
    }

    pub fn group_detail(&self, group_id: String) -> Option<GroupDetailDto> {
        self.lock()
            .group_detail(&GroupId::new(group_id))
            .map(Into::into)
    }

    pub fn friends(&self) -> Vec<FriendBalanceDto> {
        self.lock().friends().into_iter().map(Into::into).collect()
    }

    pub fn friend_detail(&self, user_id: String) -> Option<FriendDetailDto> {
        self.lock()
            .friend_detail(&UserId::new(user_id))
            .map(Into::into)
    }

    pub fn activity(&self) -> Vec<ActivityEntryDto> {
        self.lock().activity().into_iter().map(Into::into).collect()
    }

    // --- devices (#16) -----------------------------------------------------

    /// This device's public key (hex) — share it to enrol from another device.
    pub fn my_device_public(&self) -> String {
        hex32(&self.lock().my_device_public())
    }

    pub fn list_devices(&self) -> Vec<DeviceViewDto> {
        self.lock().devices().into_iter().map(Into::into).collect()
    }

    /// Authorize another device (hex public key) to act for this identity.
    pub fn authorize_device(&self, device_hex: String, site: u64) -> Result<()> {
        self.lock()
            .authorize_device(parse_hex32(&device_hex)?, site)?;
        Ok(())
    }

    /// Revoke a device (hex public key).
    pub fn revoke_device(&self, device_hex: String) -> Result<()> {
        self.lock().revoke_device(parse_hex32(&device_hex)?)?;
        Ok(())
    }

    // --- internals ---------------------------------------------------------

    fn lock(&self) -> std::sync::MutexGuard<'_, App<RedbOpStore>> {
        self.inner.lock().expect("engine mutex poisoned")
    }
}

fn seed32(bytes: &[u8], what: &str) -> Result<[u8; 32]> {
    if bytes.len() != 32 {
        return Err(anyhow!("{what} must be 32 bytes, got {}", bytes.len()));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(bytes);
    Ok(seed)
}

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_hex32(s: &str) -> Result<[u8; 32]> {
    if s.len() != 64 {
        return Err(anyhow!("expected 64 hex chars, got {}", s.len()));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| anyhow!("invalid hex"))?;
    }
    Ok(out)
}

/// Convert `amount_cents` from `from_currency` to `to_currency` at `rate_micro`
/// (target units per source unit, ×1e6). Integer-safe; the money math lives in
/// Rust so the shell never duplicates it (#3).
pub fn convert_currency(
    amount_cents: i64,
    rate_micro: i64,
    from_currency: String,
    to_currency: String,
) -> i64 {
    splittr_app::convert(
        Cents(amount_cents),
        rate_micro as u64,
        splittr_app::minor_units(&from_currency),
        splittr_app::minor_units(&to_currency),
    )
    .0
}

/// The number of minor units (decimal places) for a currency code — for
/// formatting amounts on the shell side (#3).
pub fn currency_minor_units(code: String) -> u32 {
    splittr_app::minor_units(&code)
}

/// The BIP39 recovery phrase for a 32-byte identity seed (#34).
pub fn recovery_phrase(identity_seed: Vec<u8>) -> Result<String> {
    Ok(splittr_app::recovery_phrase(&seed32(
        &identity_seed,
        "identity seed",
    )?))
}

/// Re-derive the 32-byte identity seed from a recovery phrase (#34).
pub fn seed_from_recovery_phrase(phrase: String) -> Result<Vec<u8>> {
    splittr_app::seed_from_phrase(&phrase)
        .map(|s| s.to_vec())
        .ok_or_else(|| anyhow!("invalid recovery phrase"))
}

/// Seal a 32-byte root seed under the app-lock `passphrase` for at-rest storage
/// on the primary device (#34/ADR-0005). Returns the opaque sealed blob.
pub fn seal_root_seed(passphrase: String, seed: Vec<u8>) -> Result<Vec<u8>> {
    Ok(splittr_app::seal_seed(
        &passphrase,
        &seed32(&seed, "root seed")?,
    ))
}

/// Open a [`seal_root_seed`] blob. `None` if the passphrase is wrong or the blob
/// was tampered with (#34).
pub fn open_root_seed(passphrase: String, blob: Vec<u8>) -> Option<Vec<u8>> {
    splittr_app::open_seed(&passphrase, &blob).map(|s| s.to_vec())
}

/// Decode and verify a shared invite blob (#7). Returns the inviter, context,
/// and whether it is currently valid (signature ok and not expired as of
/// `now_ms`, Unix ms); `None` if the blob is malformed.
pub fn verify_invite(invite: Vec<u8>, now_ms: u64) -> Option<InviteDto> {
    let invite = Invite::from_bytes(&invite)?;
    Some(InviteDto {
        inviter: user_id_for(&invite.inviter).to_string(),
        context: invite.context.clone(),
        expiry_ms: invite.expiry_ms,
        valid: invite.verify() && !invite.is_expired(now_ms),
    })
}

/// The device-enrolment short authentication string both devices compare to
/// defeat a man-in-the-middle (#35). All keys are 32-byte hex; `challenge` is
/// the 32-byte one-time pairing nonce.
pub fn pairing_short_auth_string(
    identity_hex: String,
    primary_device_hex: String,
    new_device_hex: String,
    challenge: Vec<u8>,
) -> Result<String> {
    let transcript = PairingTranscript {
        identity: PublicKey(parse_hex32(&identity_hex)?),
        primary_device: PublicKey(parse_hex32(&primary_device_hex)?),
        new_device: PublicKey(parse_hex32(&new_device_hex)?),
        challenge: seed32(&challenge, "challenge")?,
    };
    Ok(transcript.short_auth_string())
}
