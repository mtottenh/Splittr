//! A per-device Hybrid Logical Clock generator (ADR-0001 §HLC).

use std::time::{SystemTime, UNIX_EPOCH};

use splittr_crdt::{Hlc, SiteId};

/// Generates monotonic HLCs for the local device.
pub struct HlcGenerator {
    site: SiteId,
    last: Hlc,
}

impl HlcGenerator {
    pub fn new(site: SiteId) -> Self {
        Self {
            site,
            last: Hlc {
                wall_ms: 0,
                counter: 0,
                site,
            },
        }
    }

    /// Advance the clock against an explicit wall time. Pure and deterministic,
    /// which is what the tests use.
    pub fn next(&mut self, wall_ms: u64) -> Hlc {
        let (wall_ms, counter) = if wall_ms > self.last.wall_ms {
            (wall_ms, 0)
        } else {
            (self.last.wall_ms, self.last.counter + 1)
        };
        self.last = Hlc {
            wall_ms,
            counter,
            site: self.site,
        };
        self.last
    }

    /// Advance against the system clock.
    pub fn now(&mut self) -> Hlc {
        let wall_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.next(wall_ms)
    }
}
