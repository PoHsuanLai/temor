//! `watch_step`: turns polls of the local copy into saves. Editors save in odd ways (write to
//! a temporary file and rename it over, truncate then write), so a change only counts once the
//! file looks the same on two polls in a row, and a poll that finds no file is skipped.

use super::model::EditIn;
use temor_ssh::ByteCount;

/// How the local copy looked on one poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalStamp {
    /// Modified time in nanoseconds since the epoch.
    pub mtime_nanos: u64,
    pub size: ByteCount,
}

/// What the watch has seen of the local copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Watch {
    /// The file is as it was when last reported (or when the watch started).
    Idle { settled: LocalStamp },
    /// The file differs from `settled` and is being given a poll to hold still.
    Settling {
        settled: LocalStamp,
        seen: LocalStamp,
    },
}

impl Watch {
    /// Starts watching a copy that looks like `stamp` now. Call it again after the app itself
    /// rewrites the copy (a fresh fetch), so its own write is not reported as a save.
    pub fn new(stamp: LocalStamp) -> Self {
        Self::Idle { settled: stamp }
    }
}

/// The watch after one poll (`None` when the file could not be read, for instance between an
/// editor's delete and rename), and the save to feed to `edit_step` when one has settled.
pub fn watch_step(watch: Watch, polled: Option<LocalStamp>) -> (Watch, Option<EditIn>) {
    let Some(now) = polled else {
        return (watch, None);
    };
    match watch {
        Watch::Idle { settled } if now == settled => (watch, None),
        Watch::Idle { settled } => (Watch::Settling { settled, seen: now }, None),
        Watch::Settling { settled, .. } if now == settled => (Watch::Idle { settled }, None),
        Watch::Settling { seen, .. } if now == seen => {
            (Watch::Idle { settled: seen }, Some(EditIn::LocalSaved))
        }
        Watch::Settling { settled, .. } => (Watch::Settling { settled, seen: now }, None),
    }
}
