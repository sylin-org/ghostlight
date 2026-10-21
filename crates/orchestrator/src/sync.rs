//! Locking for the service's shared state, so that one panicking holder cannot fail every later
//! call.
//!
//! A mutex left poisoned by a holder that panicked still guards state the service can use: the
//! service replaces or reads that state whole, never half way through a change it must undo. So a
//! poisoned lock is taken as it is. Every module of this crate locks through [`lock`].

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Lock `mutex`, taking its state back from a holder that panicked.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
