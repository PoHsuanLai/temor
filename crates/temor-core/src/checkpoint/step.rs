//! `checkpoint_step`: snapshots before agent writes, and putting files back to one.

use super::model::{CheckpointPhase, Checkpoints, Mark};
use crate::vocab::ids::{BlockId, CheckpointRef};

/// What moves the checkpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckpointIn {
    /// An agent is about to write, inside `block`.
    AgentWillWrite(BlockId),
    /// The person chose "Undo all".
    UndoAll,
    /// The person chose "Undo to here" on `block`.
    UndoTo(BlockId),
}

/// What the checkpoints want done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckpointOut {
    /// Snapshot tracked and untracked files to this ref before the write goes ahead.
    Take(CheckpointRef),
    /// Put the files back to this ref, and delete the refs after it.
    Restore(CheckpointRef),
}

/// The checkpoints after `input`. A block takes one snapshot, before its first write; undoing to
/// a block keeps the marks before it.
pub fn checkpoint_step(
    state: Checkpoints,
    input: CheckpointIn,
) -> (Checkpoints, Vec<CheckpointOut>) {
    match input {
        CheckpointIn::AgentWillWrite(block)
            if state.marks.iter().any(|mark| mark.block == block) =>
        {
            (state, vec![])
        }
        CheckpointIn::AgentWillWrite(block) => {
            let at = state.next;
            let mut marks = state.marks;
            marks.push(Mark { block, at });
            let next = Checkpoints {
                phase: CheckpointPhase::Taken,
                marks,
                next: CheckpointRef(at.0 + 1),
            };
            (next, vec![CheckpointOut::Take(at)])
        }
        CheckpointIn::UndoAll => match state.marks.first().copied() {
            Some(first) => restore(state, 0, first.at),
            None => (state, vec![]),
        },
        CheckpointIn::UndoTo(block) => {
            match state.marks.iter().position(|mark| mark.block == block) {
                Some(index) => {
                    let at = state.marks[index].at;
                    restore(state, index, at)
                }
                None => (state, vec![]),
            }
        }
    }
}

fn restore(
    state: Checkpoints,
    keep: usize,
    at: CheckpointRef,
) -> (Checkpoints, Vec<CheckpointOut>) {
    let mut marks = state.marks;
    marks.truncate(keep);
    let next = Checkpoints {
        phase: CheckpointPhase::RestoredTo(at),
        marks,
        next: state.next,
    };
    (next, vec![CheckpointOut::Restore(at)])
}
