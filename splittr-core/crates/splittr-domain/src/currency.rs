//! Currency metadata and integer-safe conversion (#3).
//!
//! Money stays integer minor units. A foreign-currency expense is converted to
//! the group's base currency at entry time and the **converted integer** is
//! stored, so balances stay exact and never drift if rates later change. The
//! original amount + rate are kept only for display.

use serde::{Deserialize, Serialize};

use crate::money::Cents;

/// The original (pre-conversion) amount of an expense entered in a currency
/// other than its group's base. Display-only; the expense's `total`/splits are
/// already in the base currency.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct OriginalAmount {
    /// ISO-4217 code of the entered currency (e.g. `"EUR"`).
    pub currency: String,
    /// The entered amount, in `currency`'s own minor units.
    pub amount: Cents,
    /// Exchange rate used: base units per 1 unit of `currency`, scaled by 1e6
    /// (so 1 EUR = 1.08 USD is stored as `1_080_000`).
    pub rate_micro: u64,
}

/// Number of minor units (decimal places) for an ISO-4217 currency code.
/// Defaults to 2 for anything not specially cased.
pub fn minor_units(code: &str) -> u32 {
    match code.to_ascii_uppercase().as_str() {
        "JPY" | "KRW" | "CLP" | "ISK" | "VND" | "XAF" | "XOF" => 0,
        "BHD" | "KWD" | "OMR" | "TND" | "IQD" | "JOD" | "LYD" => 3,
        _ => 2,
    }
}

/// Convert `amount` (in `from`'s minor units) into `to`'s minor units at
/// `rate_micro` (target units per 1 source unit, ×1e6). Integer-safe with
/// round-half-away-from-zero; uses i128 internally to avoid overflow.
pub fn convert(amount: Cents, rate_micro: u64, from_minor: u32, to_minor: u32) -> Cents {
    let num = amount.0 as i128 * rate_micro as i128 * 10i128.pow(to_minor);
    let den = 1_000_000i128 * 10i128.pow(from_minor);
    Cents(round_div(num, den) as i64)
}

fn round_div(num: i128, den: i128) -> i128 {
    let half = den / 2;
    if num >= 0 {
        (num + half) / den
    } else {
        -(((-num) + half) / den)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minor_units_known_currencies() {
        assert_eq!(minor_units("USD"), 2);
        assert_eq!(minor_units("eur"), 2);
        assert_eq!(minor_units("JPY"), 0);
        assert_eq!(minor_units("BHD"), 3);
        assert_eq!(minor_units("ZZZ"), 2); // unknown → default
    }

    #[test]
    fn converts_same_minor_units() {
        // €10.00 at 1 EUR = 1.08 USD → $10.80.
        assert_eq!(convert(Cents(1000), 1_080_000, 2, 2), Cents(1080));
    }

    #[test]
    fn converts_from_zero_decimal_currency() {
        // ¥1000 at 1 JPY = 0.0067 USD → $6.70 (minor units 0 → 2).
        assert_eq!(convert(Cents(1000), 6_700, 0, 2), Cents(670));
    }

    #[test]
    fn converts_to_zero_decimal_currency() {
        // $5.00 at 1 USD = 150 JPY → ¥750 (minor units 2 → 0).
        assert_eq!(convert(Cents(500), 150_000_000, 2, 0), Cents(750));
    }

    #[test]
    fn rounds_to_nearest_minor_unit() {
        // €10.00 at 1.085 → $10.85.
        assert_eq!(convert(Cents(1000), 1_085_000, 2, 2), Cents(1085));
        // €3.33 at 1.005 → 334.665 → 335 cents.
        assert_eq!(convert(Cents(333), 1_005_000, 2, 2), Cents(335));
    }

    #[test]
    fn identity_rate_is_a_noop() {
        assert_eq!(convert(Cents(4242), 1_000_000, 2, 2), Cents(4242));
    }
}
