//! The sessions a daemon holds.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use temor_wire::ids::SessionId;

use super::session::Session;
use crate::sync::lock;

/// How many ids one start of a daemon can hand out before the next second's start could reach
/// them: a million sessions.
const IDS_PER_SECOND: u64 = 1 << 20;

/// Where a daemon started at `secs` (since the Unix epoch) numbers its sessions from. A later
/// start numbers past every id an earlier one handed out (unless that one made more than
/// [`IDS_PER_SECOND`] sessions for each second it ran), so an id a window kept from an earlier
/// daemon names no session of this one: resuming it fails, and the window starts a new shell.
pub(super) fn id_base(secs: u64) -> u64 {
    secs.saturating_mul(IDS_PER_SECOND)
}

/// The live sessions by id. `Send` and `Sync`.
#[derive(Debug)]
pub(super) struct Registry {
    sessions: Mutex<BTreeMap<SessionId, Arc<Session>>>,
    last: AtomicU64,
}

impl Default for Registry {
    /// Numbers from now (see [`id_base`]).
    fn default() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        Self::from(id_base(secs))
    }
}

impl From<u64> for Registry {
    /// A registry whose first id is `base + 1`.
    fn from(base: u64) -> Self {
        Self {
            sessions: Mutex::new(BTreeMap::new()),
            last: AtomicU64::new(base),
        }
    }
}

impl Registry {
    /// An id never used before by this daemon, nor by one started earlier.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_daemon_numbers_past_an_earlier_ones_ids() {
        let earlier = Registry::from(id_base(1_700_000_000));
        let first = earlier.next_id();
        let mut last = first;
        for _ in 0..1000 {
            last = earlier.next_id();
        }
        assert_eq!(last.0, first.0 + 1000);
        let later = Registry::from(id_base(1_700_000_001));
        assert!(later.next_id() > last, "a restart a second later");
        assert!(
            Registry::default().next_id().0 > id_base(1_700_000_000),
            "now is later"
        );
    }
}
