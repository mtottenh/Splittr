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
}

impl core::ops::Add for Cents {
    type Output = Cents;
    fn add(self, o: Cents) -> Cents {
        Cents(self.0 + o.0)
    }
}
impl core::ops::Sub for Cents {
    type Output = Cents;
    fn sub(self, o: Cents) -> Cents {
        Cents(self.0 - o.0)
    }
}
impl core::ops::Neg for Cents {
    type Output = Cents;
    fn neg(self) -> Cents {
        Cents(-self.0)
    }
}
impl core::ops::AddAssign for Cents {
    fn add_assign(&mut self, o: Cents) {
        self.0 += o.0;
    }
}
impl core::ops::SubAssign for Cents {
    fn sub_assign(&mut self, o: Cents) {
        self.0 -= o.0;
    }
}
