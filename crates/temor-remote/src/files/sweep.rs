//! Clearing out staged parts that nobody finished. A streamed write stages in a hidden file
//! beside its target; one whose writer vanished (the link died, the window closed) stays
//! behind. The daemon sweeps the old ones when it starts.

use std::time::Duration;

use temor_wire::message::EntryKind;
use temor_wire::units::UnixMillis;

use super::service::FileService;
use super::write::STAGE_PREFIX;
use crate::platform::Beneath;

/// A staged file this old has no writer left.
pub const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// How deep below a granted folder the sweep looks.
const DEPTH: usize = 8;

/// How many entries the sweep reads before it stops, so a huge home folder cannot hold the
/// daemon's start.
const BUDGET: usize = 50_000;

/// Whether an entry is a staged file older than `older_than` at `now`. A file stamped in the
/// future is not old.
pub fn is_stale_stage(
    name: &str,
    kind: EntryKind,
    mtime: UnixMillis,
    now: UnixMillis,
    older_than: Duration,
) -> bool {
    let limit = i64::try_from(older_than.as_millis()).unwrap_or(i64::MAX);
    name.starts_with(STAGE_PREFIX)
        && kind == EntryKind::File
        && now.0.saturating_sub(mtime.0) > limit
}

impl FileService {
    /// Removes the staged files older than `older_than` that lie in the granted folders, up to
    /// [`DEPTH`] folders down and without crossing a link. The number removed. Whatever cannot
    /// be read or removed is left for the next start.
    pub fn sweep_staged(&self, now: UnixMillis, older_than: Duration) -> usize {
        let mut budget = BUDGET;
        let mut removed = 0;
        for root in self.roots() {
            let Ok(beneath) = Beneath::open(&root) else {
                continue;
            };
            let shown = root.display().to_string();
            removed += sweep_folder(&beneath, &[], &shown, (now, older_than), &mut budget);
        }
        removed
    }
}

fn sweep_folder(
    beneath: &Beneath,
    rel: &[String],
    shown: &str,
    (now, older_than): (UnixMillis, Duration),
    budget: &mut usize,
) -> usize {
    let Ok(entries) = beneath.read_dir(rel, shown) else {
        return 0;
    };
    let mut removed = 0;
    for (name, stat) in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let mut below = rel.to_vec();
        below.push(name.clone());
        if is_stale_stage(&name, stat.kind, stat.mtime, now, older_than) {
            // `discard_part` works beside the last name of the path it is given.
            if beneath.discard_part(&below, &name, shown).is_ok() {
                removed += 1;
            }
        } else if stat.kind == EntryKind::Directory && rel.len() < DEPTH {
            removed += sweep_folder(beneath, &below, shown, (now, older_than), budget);
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    #[test]
    fn only_old_staged_files_are_stale() {
        let now = UnixMillis(10 * DAY_MS);
        let staged = ".temor-part-0123456789abcdef";
        // Name, entry name, kind, age in ms, stale?
        let cases = [
            (
                "a day and a second",
                staged,
                EntryKind::File,
                DAY_MS + 1000,
                true,
            ),
            ("exactly a day", staged, EntryKind::File, DAY_MS, false),
            ("an hour", staged, EntryKind::File, 3_600_000, false),
            ("from the future", staged, EntryKind::File, -5000, false),
            (
                "old but the person's own file",
                "notes.txt",
                EntryKind::File,
                9 * DAY_MS,
                false,
            ),
            (
                "old hidden file of another kind",
                ".bashrc",
                EntryKind::File,
                9 * DAY_MS,
                false,
            ),
            (
                "a folder with the name",
                staged,
                EntryKind::Directory,
                9 * DAY_MS,
                false,
            ),
            (
                "a link with the name",
                staged,
                EntryKind::Symlink,
                9 * DAY_MS,
                false,
            ),
        ];
        for (why, name, kind, age, want) in cases {
            let mtime = UnixMillis(now.0 - age);
            assert_eq!(
                is_stale_stage(name, kind, mtime, now, STALE_AFTER),
                want,
                "{why}"
            );
        }
    }
}
