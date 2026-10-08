use super::{CheckpointIn, CheckpointOut, CheckpointPhase, Checkpoints, Mark, checkpoint_step};
use crate::vocab::ids::{BlockId, CheckpointRef};

fn marks(pairs: &[(u64, u64)]) -> Vec<Mark> {
    pairs
        .iter()
        .map(|&(block, at)| Mark {
            block: BlockId(block),
            at: CheckpointRef(at),
        })
        .collect()
}
fn taken(pairs: &[(u64, u64)], next: u64) -> Checkpoints {
    Checkpoints {
        phase: CheckpointPhase::Taken,
        marks: marks(pairs),
        next: CheckpointRef(next),
    }
}

#[test]
fn checkpoints_are_taken_per_block_and_restored() {
    let restored = |at, pairs: &[(u64, u64)]| Checkpoints {
        phase: CheckpointPhase::RestoredTo(CheckpointRef(at)),
        marks: marks(pairs),
        next: CheckpointRef(3),
    };
    #[rustfmt::skip]
    let cases: Vec<(&str, Checkpoints, CheckpointIn, Checkpoints, Vec<CheckpointOut>)> = vec![
        ("the first write takes one", Checkpoints::default(), CheckpointIn::AgentWillWrite(BlockId(5)),
            taken(&[(5, 0)], 1), vec![CheckpointOut::Take(CheckpointRef(0))]),
        ("a block's second write takes none", taken(&[(5, 0)], 1), CheckpointIn::AgentWillWrite(BlockId(5)),
            taken(&[(5, 0)], 1), vec![]),
        ("a new block takes another", taken(&[(5, 0)], 1), CheckpointIn::AgentWillWrite(BlockId(6)),
            taken(&[(5, 0), (6, 1)], 2), vec![CheckpointOut::Take(CheckpointRef(1))]),
        ("undo all goes to the first", taken(&[(5, 0), (6, 1), (7, 2)], 3), CheckpointIn::UndoAll,
            restored(0, &[]), vec![CheckpointOut::Restore(CheckpointRef(0))]),
        ("undo to here keeps the earlier marks", taken(&[(5, 0), (6, 1), (7, 2)], 3), CheckpointIn::UndoTo(BlockId(6)),
            restored(1, &[(5, 0)]), vec![CheckpointOut::Restore(CheckpointRef(1))]),
        ("undo to an unknown block does nothing", taken(&[(5, 0)], 1), CheckpointIn::UndoTo(BlockId(9)),
            taken(&[(5, 0)], 1), vec![]),
        ("undo all with none does nothing", Checkpoints::default(), CheckpointIn::UndoAll, Checkpoints::default(), vec![]),
        ("a write after a restore never reuses a ref", restored(1, &[(5, 0)]), CheckpointIn::AgentWillWrite(BlockId(8)),
            taken(&[(5, 0), (8, 3)], 4), vec![CheckpointOut::Take(CheckpointRef(3))]),
    ];
    for (name, from, input, want, outs) in cases {
        let (state, out) = checkpoint_step(from, input);
        assert_eq!(state, want, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
    }
}
