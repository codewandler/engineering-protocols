//! Timestamps.
//!
//! The domain crate is deliberately clock-free: a [`Timestamp`] can be constructed from an
//! epoch value but never read from the system clock here. Wall-clock access belongs to the
//! engine, behind a `Clock` it can swap for a fixed one in tests, which is what makes an
//! execution replayable.

use std::fmt;

/// Milliseconds since the Unix epoch, UTC.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    /// The epoch itself, useful as a deterministic default in tests.
    pub const EPOCH: Self = Self(0);

    /// Builds a timestamp from milliseconds since the Unix epoch.
    pub const fn from_epoch_millis(millis: u64) -> Self {
        Self(millis)
    }

    /// Milliseconds since the Unix epoch.
    pub const fn epoch_millis(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}
