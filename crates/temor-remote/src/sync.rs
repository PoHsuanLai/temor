//! Locking a mutex that a panicked thread may have poisoned.

use std::sync::{Mutex, MutexGuard};

/// The guard of `mutex`, even when a thread panicked while holding it: what these mutexes guard
/// (queues, tables, a path) stays usable, and one panicking thread must not stop the rest.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
