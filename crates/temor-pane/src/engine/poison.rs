//! Locking without giving up on a poisoned mutex.

use std::sync::{Mutex, MutexGuard};

/// Locks `mutex`. A poisoned lock still holds its value (the terminal, a queue of bytes, the
/// clipboard's answer), so the pane keeps going rather than dropping it.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
