//! An in-memory reference backend for the AEP interaction contract.
//!
//! It exists so the contract is exercised by something real before anyone builds a durable backend,
//! and so a conformance suite has a known-good implementation to check itself against.
//!
//! What it demonstrates, and what a real backend has to reproduce:
//!
//! | behaviour | where |
//! |---|---|
//! | a replayed command returns its original result, and applies nothing twice | [`command`] |
//! | `expected_revision` mismatches are refused, never merged | [`command`] |
//! | a refused command leaves no state change and still leaves an audit record | [`command`] |
//! | nothing is ever physically deleted | [`command`] |
//! | reads can demand a view no older than a given write | [`query`] |
//!
//! It is not a persistence layer and makes no attempt to be one: everything lives in `BTreeMap`s
//! behind one lock.

pub mod command;
pub mod query;
pub mod seed;
pub mod store;

use std::sync::Mutex;

pub use store::{AppliedCommand, StoredEntity};

/// The backend.
#[derive(Debug, Default)]
pub struct MemoryBackend {
    store: Mutex<store::Store>,
}

impl MemoryBackend {
    /// An empty backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `read` against the store.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned, which means a previous call panicked while holding it — the
    /// store's contents cannot be trusted after that, and continuing would hide the original fault.
    pub fn with_store<T>(&self, read: impl FnOnce(&store::Store) -> T) -> T {
        let store = self.store.lock().expect("the store lock is not poisoned");
        read(&store)
    }

    /// Runs `write` against the store.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned; see [`MemoryBackend::with_store`].
    pub fn with_store_mut<T>(&self, write: impl FnOnce(&mut store::Store) -> T) -> T {
        let mut store = self.store.lock().expect("the store lock is not poisoned");
        write(&mut store)
    }

    /// How many entities are stored.
    pub fn len(&self) -> usize {
        self.with_store(store::Store::len)
    }

    /// `true` when nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.with_store(store::Store::is_empty)
    }
}
