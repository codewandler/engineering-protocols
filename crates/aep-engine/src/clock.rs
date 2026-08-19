//! Time, injected.
//!
//! The engine reads the clock only through this trait. That is what makes an execution replayable:
//! given the same plan, the same evidence and a [`FixedClock`], the event stream is byte-identical,
//! which is the difference between an audit trail you can diff and one you can only read.

use std::sync::atomic::{AtomicU64, Ordering};

use aep_domain::Timestamp;

/// A source of the current time.
pub trait Clock: Send + Sync {
    /// The current time.
    fn now(&self) -> Timestamp;
}

/// The wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        Timestamp::from_epoch_millis(millis)
    }
}

/// A clock that returns the same instant every time, for tests and replay.
#[derive(Debug)]
pub struct FixedClock {
    millis: u64,
}

impl FixedClock {
    /// A clock stuck at `millis` since the epoch.
    pub fn new(millis: u64) -> Self {
        Self { millis }
    }
}

impl Default for FixedClock {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_epoch_millis(self.millis)
    }
}

/// A clock that advances by a fixed step on every read.
///
/// Useful when a test needs distinguishable timestamps without depending on how fast the machine is.
#[derive(Debug)]
pub struct SteppingClock {
    next: AtomicU64,
    step: u64,
}

impl SteppingClock {
    /// A clock starting at `start` and advancing `step` milliseconds per read.
    pub fn new(start: u64, step: u64) -> Self {
        Self {
            next: AtomicU64::new(start),
            step,
        }
    }
}

impl Default for SteppingClock {
    fn default() -> Self {
        Self::new(1_000, 1_000)
    }
}

impl Clock for SteppingClock {
    fn now(&self) -> Timestamp {
        let millis = self.next.fetch_add(self.step, Ordering::Relaxed);
        Timestamp::from_epoch_millis(millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_clock_never_moves() {
        let clock = FixedClock::new(42);
        assert_eq!(clock.now(), clock.now());
        assert_eq!(clock.now().epoch_millis(), 42);
    }

    #[test]
    fn a_stepping_clock_moves_by_its_step() {
        let clock = SteppingClock::new(10, 5);
        assert_eq!(clock.now().epoch_millis(), 10);
        assert_eq!(clock.now().epoch_millis(), 15);
        assert_eq!(clock.now().epoch_millis(), 20);
    }

    #[test]
    fn the_system_clock_is_after_2020() {
        let clock = SystemClock;
        assert!(clock.now().epoch_millis() > 1_577_836_800_000);
    }
}
