//! Where a connection's output goes: each attached session has a pane's [`Feeder`]. Output that
//! arrives before the pane has its screen (the snapshot comes first) waits here, so nothing
//! lands under the screen that replaces it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use temor_pane::Feeder;
use temor_pty::ChildExit;
use temor_vt::Snapshot;
use temor_wire::ids::RemoteSessionId;

/// The most output kept for a session whose pane has not taken its screen yet (1 MiB).
pub const BACKLOG_CAP: usize = 1 << 20;

/// Output waiting for a screen. Past the cap the oldest bytes go: the screen that follows
/// replaces most of it anyway.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Backlog(Vec<u8>);

impl Backlog {
    /// Adds `bytes`, keeping at most [`BACKLOG_CAP`] of the newest.
    pub fn push(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
        if self.0.len() > BACKLOG_CAP {
            let excess = self.0.len() - BACKLOG_CAP;
            self.0.drain(..excess);
        }
    }

    /// Everything waiting, leaving it empty.
    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}

#[derive(Debug, Default)]
struct Route {
    feeder: Option<Feeder>,
    backlog: Backlog,
}

/// The sessions of one connection and the panes that show them. Cloning shares the table.
#[derive(Debug, Clone, Default)]
pub struct Router(Arc<Mutex<BTreeMap<RemoteSessionId, Route>>>);

impl Router {
    fn table(&self) -> MutexGuard<'_, BTreeMap<RemoteSessionId, Route>> {
        // A poisoned table still holds the routes; output keeps flowing.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Output of `session`: to its pane, or to the backlog until the pane has its screen.
    pub fn output(&self, session: RemoteSessionId, bytes: Vec<u8>) {
        let mut table = self.table();
        let route = table.entry(session).or_default();
        match &route.feeder {
            Some(feeder) => feeder.bytes(bytes),
            None => route.backlog.push(&bytes),
        }
    }

    /// The pane shows `session` from `screen` on; what arrived meanwhile follows it.
    pub fn attach(&self, session: RemoteSessionId, feeder: Feeder, screen: Snapshot) {
        let mut table = self.table();
        let route = table.entry(session).or_default();
        feeder.restore(screen);
        feeder.bytes(route.backlog.take());
        route.feeder = Some(feeder);
    }

    /// The session's program ended: its pane says so, and the route goes.
    pub fn ended(&self, session: RemoteSessionId, exit: Option<ChildExit>) {
        if let Some(Route {
            feeder: Some(feeder),
            ..
        }) = self.table().remove(&session)
        {
            feeder.end(exit);
        }
    }

    /// Forgets `session` (its pane closed).
    pub fn forget(&self, session: RemoteSessionId) {
        self.table().remove(&session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backlog_keeps_the_newest_bytes_up_to_the_cap() {
        let mut backlog = Backlog::default();
        backlog.push(&vec![b'a'; BACKLOG_CAP]);
        backlog.push(b"tail");
        let kept = backlog.take();
        assert_eq!(kept.len(), BACKLOG_CAP);
        assert!(
            kept.ends_with(b"tail"),
            "the oldest bytes went, not the newest"
        );
        assert_eq!(backlog.take(), Vec::<u8>::new(), "taking empties it");
    }
}
