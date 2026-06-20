//! Logical clocks and replica identifiers.

use serde::{Deserialize, Serialize};

/// A replica/device identifier; the HLC tiebreaker and (later) the signing
/// device key (#16).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SiteId(pub u64);

/// Hybrid Logical Clock. Field declaration order defines the derived total
/// order (compare `wall_ms`, then `counter`, then `site`) — ADR-0001 §HLC.
/// HLCs give a causality-respecting deterministic order without synchronized
/// clocks; they are used only as tiebreakers in last-writer-wins registers.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Hlc {
    pub wall_ms: u64,
    pub counter: u32,
    pub site: SiteId,
}
