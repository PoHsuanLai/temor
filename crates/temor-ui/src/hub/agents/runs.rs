//! An agent run's checkpoints (D9, §8.2): a snapshot before each turn of an agent pane, kept in
//! hidden refs of the repository the pane is in. The numbering is seeded from what the repository
//! already has, so two runs never overwrite each other. Pure: the git work is `work.rs`.

use temor_core::checkpoint::{CheckpointIn, CheckpointOut, Checkpoints, Mark, checkpoint_step};
use temor_core::vocab::ids::{BlockId, CheckpointRef};

use super::work::Seeded;

/// One pane's run.
#[derive(Debug, Clone, Default)]
pub struct Run {
    /// Where the checkpoints stand.
    pub checkpoints: Checkpoints,
    /// The repository they are in.
    pub root: Option<String>,
    /// The number of the last turn started; a turn is a block of the checkpoint machine.
    turn: u64,
    /// Turns waiting for the repository to be read.
    pending: Vec<BlockId>,
}

impl Run {
    /// An agent turn started: the folder is read again (the pane may have moved), then the
    /// turn's snapshot is taken.
    pub fn turn_started(&mut self) {
        self.turn += 1;
        self.pending.push(BlockId(self.turn));
    }

    /// The repository was read. Returns the snapshots to take now. A folder outside a repository
    /// drops the waiting turns; a different repository starts the checkpoints afresh.
    pub fn seeded(&mut self, seeded: &Seeded) -> Vec<CheckpointRef> {
        let blocks = std::mem::take(&mut self.pending);
        let Some(root) = &seeded.root else {
            return Vec::new();
        };
        if self.root.as_ref() != Some(root) {
            self.checkpoints = Checkpoints::default();
            self.root = Some(root.clone());
        }
        // Never behind the repository: another run may have taken numbers.
        if self.checkpoints.next.0 < seeded.next.0 {
            self.checkpoints.next = seeded.next;
        }
        let mut takes = Vec::new();
        for block in blocks {
            let (next, outs) = checkpoint_step(
                std::mem::take(&mut self.checkpoints),
                CheckpointIn::AgentWillWrite(block),
            );
            self.checkpoints = next;
            takes.extend(outs.into_iter().filter_map(|out| match out {
                CheckpointOut::Take(at) => Some(at),
                CheckpointOut::Restore(_) => None,
            }));
        }
        takes
    }

    /// A snapshot could not be taken: it is no longer offered.
    pub fn take_failed(&mut self, at: CheckpointRef) {
        self.checkpoints.marks.retain(|mark| mark.at != at);
    }

    /// The files were put back to the snapshot of `block`; the ones after it are gone.
    pub fn restored(&mut self, block: BlockId) {
        let (next, _) = checkpoint_step(
            std::mem::take(&mut self.checkpoints),
            CheckpointIn::UndoTo(block),
        );
        self.checkpoints = next;
    }

    /// The snapshots still offered, oldest first.
    pub fn marks(&self) -> &[Mark] {
        &self.checkpoints.marks
    }
}
