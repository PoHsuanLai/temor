//! A run's checkpoints: the snapshots taken and where they stand.

use crate::vocab::ids::{BlockId, CheckpointRef};

/// One snapshot: taken before `block` first wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mark {
    /// The agent block.
    pub block: BlockId,
    /// Its snapshot.
    pub at: CheckpointRef,
}

/// Where a run's checkpoints stand.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckpointPhase {
    /// None taken.
    None,
    /// Taken; the last mark is the newest.
    Taken,
    /// The files were put back to this snapshot.
    RestoredTo(CheckpointRef),
}

/// A run's checkpoints, oldest first, and the number the next one takes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Checkpoints {
    /// Where they stand.
    pub phase: CheckpointPhase,
    /// The snapshots still offered.
    pub marks: Vec<Mark>,
    /// The next snapshot's number.
    pub next: CheckpointRef,
}

impl Default for Checkpoints {
    fn default() -> Checkpoints {
        Checkpoints {
            phase: CheckpointPhase::None,
            marks: Vec::new(),
            next: CheckpointRef(0),
        }
    }
}
