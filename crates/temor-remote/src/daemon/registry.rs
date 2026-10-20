//! The sessions a daemon holds.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use temor_wire::ids::SessionId;

use super::session::Session;
use crate::sync::lock;

/// The live sessions by id. `Send` and `Sync`.
#[derive(Debug, Default)]
pub(super) struct Registry {
    sessions: Mutex<BTreeMap<SessionId, Arc<Session>>>,
    last: AtomicU64,
}

impl Registry {
    /// An id never used before by this daemon.
    pub(super) fn next_id(&self) -> SessionId {
        SessionId(self.last.fetch_add(1, Ordering::Relaxed) + 1)
    }

    pub(super) fn insert(&self, session: Arc<Session>) {
        self.lock().insert(session.id, session);
    }

    pub(super) fn get(&self, id: SessionId) -> Option<Arc<Session>> {
        self.lock().get(&id).cloned()
    }

    pub(super) fn remove(&self, id: SessionId) {
        self.lock().remove(&id);
    }

    /// Every session, by id.
    pub(super) fn all(&self) -> Vec<Arc<Session>> {
        self.lock().values().cloned().collect()
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<SessionId, Arc<Session>>> {
        lock(&self.sessions)
    }
}
