//! Watching: the daemon looks again at each watched path on every tick and reports what changed.
//! Looking is a plain `stat`, so a watch costs nothing between ticks and works on every file
//! system (inotify is a possible later refinement).

use temor_wire::ids::WatchId;
use temor_wire::message::{EntryKind, RemotePath, WatchChange};
use temor_wire::units::UnixMillis;

/// What a watch sees of a path: enough to tell that it changed, cheap to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observed {
    /// What the path is.
    pub kind: EntryKind,
    /// Its length.
    pub len: u64,
    /// Its modification time. A folder's changes when an entry is added or removed.
    pub mtime: UnixMillis,
}

/// What changed between two looks at a path, if anything.
pub fn watch_step(before: Option<Observed>, after: Option<Observed>) -> Option<WatchChange> {
    match (before, after) {
        (None, None) => None,
        (None, Some(_)) => Some(WatchChange::Created),
        (Some(_), None) => Some(WatchChange::Removed),
        (Some(a), Some(b)) if a == b => None,
        (Some(_), Some(_)) => Some(WatchChange::Modified),
    }
}

#[derive(Debug)]
struct Watched {
    id: WatchId,
    path: RemotePath,
    last: Option<Observed>,
}

/// One connection's watches.
#[derive(Debug, Default)]
pub struct WatchTable {
    next: u64,
    entries: Vec<Watched>,
}

impl WatchTable {
    /// Starts watching `path`, which looked like `last` just now (`None` if it is not there yet).
    pub fn add(&mut self, path: RemotePath, last: Option<Observed>) -> WatchId {
        self.next += 1;
        let id = WatchId(self.next);
        self.entries.push(Watched { id, path, last });
        id
    }

    /// Stops a watch; the path it watched, or `None` if there was no such watch.
    pub fn remove(&mut self, id: WatchId) -> Option<RemotePath> {
        let at = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(at).path)
    }

    /// Every watch and its path, to look at again.
    pub fn paths(&self) -> Vec<(WatchId, RemotePath)> {
        self.entries
            .iter()
            .map(|entry| (entry.id, entry.path.clone()))
            .collect()
    }

    /// Records a new look at a watched path; what changed since the last, with the path.
    pub fn apply(
        &mut self,
        id: WatchId,
        now: Option<Observed>,
    ) -> Option<(RemotePath, WatchChange)> {
        let entry = self.entries.iter_mut().find(|entry| entry.id == id)?;
        let change = watch_step(entry.last, now)?;
        entry.last = now;
        Some((entry.path.clone(), change))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(len: u64, mtime: i64) -> Option<Observed> {
        Some(Observed {
            kind: EntryKind::File,
            len,
            mtime: UnixMillis(mtime),
        })
    }

    #[test]
    fn each_pair_of_looks_names_one_change() {
        let cases = [
            (None, None, None),
            (None, seen(1, 10), Some(WatchChange::Created)),
            (seen(1, 10), None, Some(WatchChange::Removed)),
            (seen(1, 10), seen(1, 10), None),
            (seen(1, 10), seen(1, 11), Some(WatchChange::Modified)),
            (seen(1, 10), seen(2, 10), Some(WatchChange::Modified)),
        ];
        for (before, after, want) in cases {
            assert_eq!(watch_step(before, after), want, "{before:?} -> {after:?}");
        }
    }

    #[test]
    fn a_change_is_reported_once_and_a_removed_watch_stays_quiet() {
        let mut table = WatchTable::default();
        let id = table.add(RemotePath("/a".into()), seen(1, 10));
        assert_eq!(table.apply(id, seen(1, 10)), None);
        assert_eq!(
            table.apply(id, seen(5, 11)),
            Some((RemotePath("/a".into()), WatchChange::Modified))
        );
        assert_eq!(table.apply(id, seen(5, 11)), None);
        assert_eq!(table.remove(id), Some(RemotePath("/a".into())));
        assert_eq!(table.apply(id, None), None);
        assert_eq!(table.remove(id), None);
    }
}
