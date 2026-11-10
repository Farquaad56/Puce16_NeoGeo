//! Neo Geo MVS watchdog timer.
//!
//! Times out after 3,244,030 master clock cycles (~135ms).
//! Reset by writing to $300001 (watchdog kick).

/// Watchdog timeout threshold in master clock cycles.
const WATCHDOG_TIMEOUT: u64 = 3_244_030; // spec: docs/spec/05-boot-io-system.md §Watchdog

#[derive(Default)]
pub struct Watchdog {
    pub counter: u64,
}

impl Watchdog {
    /// Reset the watchdog timer.
    pub fn kick(&mut self) {
        self.counter = 0;
    }

    /// Cycles restant avant l'expiration (T900b : décider si un rattrapage peut être différé).
    pub fn headroom(&self) -> u64 {
        WATCHDOG_TIMEOUT.saturating_sub(self.counter)
    }

    /// Advance the watchdog by `mclk` master clock cycles.
    /// Returns true if the watchdog has timed out (requires machine reset).
    pub fn advance(&mut self, mclk: u64) -> bool {
        self.counter += mclk;
        self.counter >= WATCHDOG_TIMEOUT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_kick_resets_counter() {
        let mut wd = Watchdog { counter: 1000 };
        wd.kick();
        assert_eq!(wd.counter, 0);
    }

    #[test]
    fn watchdog_no_timeout_before_threshold() {
        let mut wd = Watchdog::default();
        // Advance to just before timeout
        let timed_out = wd.advance(3_244_029);
        assert!(!timed_out);
    }

    #[test]
    fn watchdog_times_out_at_threshold() {
        let mut wd = Watchdog::default();
        // Advance to just before timeout, then one more cycle
        wd.advance(3_244_029);
        let timed_out = wd.advance(1);
        assert!(timed_out);
    }

    #[test]
    fn watchdog_times_out_past_threshold() {
        let mut wd = Watchdog::default();
        // Advance well past timeout
        let timed_out = wd.advance(4_000_000);
        assert!(timed_out);
    }

    #[test]
    fn watchdog_kick_prevents_timeout() {
        let mut wd = Watchdog::default();
        wd.advance(3_244_000);
        wd.kick();
        // Now advance again - should not timeout immediately
        let timed_out = wd.advance(100);
        assert!(!timed_out);
    }
}
