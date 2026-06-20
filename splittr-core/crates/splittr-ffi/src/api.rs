//! The FFI API surface scanned by flutter_rust_bridge.
//!
//! [`Engine`] is an opaque handle Dart holds: it owns the local [`App`] behind a
//! `Mutex` (so the handle is shared and `Sync`), and every method locks, calls
//! the app, and marshals results to DTOs. Errors surface as `anyhow::Error`,
//! which FRB maps to a Dart exception.

use std::sync::Mutex;

use anyhow::{anyhow, Result};
use splittr_app::{App, Cents, GroupId, Identity, RedbOpStore, SettlementId, SiteId, UserId};

use crate::convert::{to_original, to_paid_by, to_split_plan};
use crate::dto::{
    ActivityEntryDto, ExpenseInput, FriendBalanceDto, FriendDetailDto, GroupDetailDto,
    GroupSummaryDto,
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
        db_key: Vec<u8>,
        site: u64,
    ) -> Result<Engine> {
        let seed = seed32(&identity_seed, "identity seed")?;
        let key = seed32(&db_key, "db key")?;
        let store = RedbOpStore::open_encrypted(&db_path, key)?;
        let app = App::new(Identity::from_seed(seed), store, SiteId(site))?;
        Ok(Engine {
            inner: Mutex::new(app),
        })
    }

    // --- profiles & people -------------------------------------------------

    pub fn my_user_id(&self) -> String {
        self.lock().me().to_string()
    }

    pub fn my_name(&self) -> Option<String> {
        self.lock().my_name()
    }

    pub fn set_my_name(&self, name: String) -> Result<()> {
        self.lock().set_my_name(&name)?;
        Ok(())
    }

    pub fn add_person(&self, name: String) -> Result<String> {
        Ok(self.lock().add_person(&name)?.0)
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
        let paid_by = to_paid_by(input.paid_by);
        let plan = to_split_plan(input.split);
        let total = Cents(input.total_cents);
        let original = to_original(input.original);
        let mut app = self.lock();
        let id = match input.group_id {
            Some(group_id) => {
                let group = GroupId::new(group_id);
                if input.draft {
                    app.add_draft_expense(
                        &group,
                        &input.description,
                        paid_by,
                        total,
                        plan,
                        &input.category,
                        input.notes,
                        input.date_ms,
                        original,
                    )?
                } else {
                    app.add_expense(
                        &group,
                        &input.description,
                        paid_by,
                        total,
                        plan,
                        &input.category,
                        input.notes,
                        input.date_ms,
                        original,
                    )?
                }
            }
            None => app.add_non_group_expense(
                &input.description,
                paid_by,
                total,
                plan,
                &input.category,
                input.notes,
                input.date_ms,
                original,
                input.draft,
            )?,
        };
        Ok(id.0)
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

    /// Replace an expense with a new version. `input.group_id` is ignored (an
    /// expense cannot change groups).
    pub fn edit_expense(&self, expense_id: String, input: ExpenseInput) -> Result<()> {
        let paid_by = to_paid_by(input.paid_by);
        let plan = to_split_plan(input.split);
        let original = to_original(input.original);
        self.lock().edit_expense(
            &splittr_app::ExpenseId::new(expense_id),
            &input.description,
            paid_by,
            Cents(input.total_cents),
            plan,
            &input.category,
            input.notes,
            input.date_ms,
            original,
        )?;
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
