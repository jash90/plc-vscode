//! Deterministic virtual time, decoupled from wall-clock time.
//!
//! Timers and the scan loop read time exclusively through [`VirtualClock`] so
//! simulator behavior is repeatable across platforms and tests can advance time
//! explicitly.

/// Monotonic virtual clock measured in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualClock {
    now_ms: i64,
    scan_interval_ms: i64,
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self {
            now_ms: 0,
            scan_interval_ms: 10,
        }
    }
}

impl VirtualClock {
    pub fn with_scan_interval_ms(scan_interval_ms: i64) -> Self {
        Self {
            now_ms: 0,
            scan_interval_ms,
        }
    }

    /// Current virtual time in milliseconds.
    pub fn now_ms(&self) -> i64 {
        self.now_ms
    }

    /// Per-scan time increment.
    pub fn scan_interval_ms(&self) -> i64 {
        self.scan_interval_ms
    }

    pub fn set_scan_interval_ms(&mut self, scan_interval_ms: i64) {
        self.scan_interval_ms = scan_interval_ms;
    }

    /// Advance virtual time by one scan interval.
    pub fn tick(&mut self) {
        self.now_ms += self.scan_interval_ms;
    }

    /// Advance virtual time by an explicit number of milliseconds.
    pub fn advance(&mut self, delta_ms: i64) {
        self.now_ms += delta_ms;
    }
}
