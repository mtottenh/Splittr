//! Money as integer minor units (cents). Integer arithmetic avoids the rounding
//! errors that plague floating-point money; splits reconcile to the penny.

use serde::{Deserialize, Serialize};

/// A monetary amount in integer minor units (cents).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Checked addition; `None` on `i64` overflow. Use on aggregation paths where
    /// a sum could in principle exceed the range (defence in depth — real ledgers
    /// never approach ~9.2e16 cents).
    pub fn checked_add(self, o: Cents) -> Option<Cents> {
        self.0.checked_add(o.0).map(Cents)
    }

    /// Checked subtraction; `None` on `i64` overflow.
    pub fn checked_sub(self, o: Cents) -> Option<Cents> {
        self.0.checked_sub(o.0).map(Cents)
    }

    /// Saturating addition — clamps at the `i64` bounds instead of overflowing.
    pub fn saturating_add(self, o: Cents) -> Cents {
        Cents(self.0.saturating_add(o.0))
    }

    /// Render against a currency's minor-unit count (2 → `"12.34"`, 0 → `"12"`,
    /// 3 → `"12.340"`). The single money-rendering helper so callers never
    /// hardcode the decimal places (see `currency::minor_units`).
    pub fn format_units(self, minor_units: u32) -> String {
        let mag = self.0.unsigned_abs();
        let body = if minor_units == 0 {
            mag.to_string()
        } else {
            let scale = 10u64.pow(minor_units);
            format!(
                "{}.{:0width$}",
                mag / scale,
                mag % scale,
                width = minor_units as usize
            )
        };
        if self.0 < 0 {
            format!("-{body}")
        } else {
            body
        }
    }
}

impl core::ops::Add for Cents {
    type Output = Cents;
    /// # Panics
    /// Panics on `i64` overflow rather than wrapping silently — a wrapped balance
    /// is a silent correctness failure in the one type meant to prevent it. Use
    /// [`Cents::checked_add`] where overflow is genuinely possible.
    fn add(self, o: Cents) -> Cents {
        Cents(
            self.0
                .checked_add(o.0)
                .expect("Cents addition overflowed i64"),
        )
    }
}
impl core::ops::Sub for Cents {
    type Output = Cents;
    /// # Panics
    /// Panics on `i64` overflow; see [`Cents::add`].
    fn sub(self, o: Cents) -> Cents {
        Cents(
            self.0
                .checked_sub(o.0)
                .expect("Cents subtraction overflowed i64"),
        )
    }
}
impl core::ops::Neg for Cents {
    type Output = Cents;
    /// # Panics
    /// Panics on `i64::MIN` (its negation is out of range).
    fn neg(self) -> Cents {
        Cents(self.0.checked_neg().expect("Cents negation overflowed i64"))
    }
}
impl core::ops::AddAssign for Cents {
    fn add_assign(&mut self, o: Cents) {
        *self = *self + o;
    }
}
impl core::ops::SubAssign for Cents {
    fn sub_assign(&mut self, o: Cents) {
        *self = *self - o;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_units_handles_each_decimal_count() {
        assert_eq!(Cents(1234).format_units(2), "12.34");
        assert_eq!(Cents(1000).format_units(0), "1000"); // ¥1000, not "10.00"
        assert_eq!(Cents(12340).format_units(3), "12.340"); // BHD
        assert_eq!(Cents(5).format_units(2), "0.05");
        assert_eq!(Cents(-1234).format_units(2), "-12.34");
        assert_eq!(Cents(0).format_units(2), "0.00");
    }

    #[test]
    fn checked_ops_report_overflow() {
        assert_eq!(Cents(1).checked_add(Cents(2)), Some(Cents(3)));
        assert_eq!(Cents(i64::MAX).checked_add(Cents(1)), None);
        assert_eq!(Cents(i64::MIN).checked_sub(Cents(1)), None);
        assert_eq!(Cents(i64::MAX).saturating_add(Cents(10)), Cents(i64::MAX));
    }

    #[test]
    #[should_panic(expected = "overflowed")]
    fn add_panics_instead_of_wrapping() {
        let _ = Cents(i64::MAX) + Cents(1);
    }
}
