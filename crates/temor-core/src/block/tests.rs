use super::{Author, Block, BlockIn, BlockOut, BlockState, block_step};
use crate::vocab::ids::{BlockId, ExitCode, GridRow, GridSpan};
use crate::vocab::text::CommandText;
use ds_core::time::stamp::Stamp;

fn a(row: u64) -> BlockIn {
    BlockIn::PromptStart {
        row: GridRow(row),
        at: Stamp(row),
    }
}
fn b(row: u64) -> BlockIn {
    BlockIn::CommandStart { row: GridRow(row) }
}
fn c(row: u64) -> BlockIn {
    BlockIn::OutputStart {
        id: BlockId(7),
        row: GridRow(row),
        command: CommandText("ls".into()),
        by: Author::Person,
        at: Stamp(100),
    }
}
fn d(row: u64, exit: Option<i32>) -> BlockIn {
    BlockIn::Finished {
        row: GridRow(row),
        exit: exit.map(ExitCode),
        at: Stamp(200),
    }
}
fn span(first: u64, last: u64) -> GridSpan {
    GridSpan {
        first: GridRow(first),
        last: GridRow(last),
    }
}
fn running() -> Block {
    Block {
        id: BlockId(7),
        prompt: span(1, 1),
        command: CommandText("ls".into()),
        output: span(2, 2),
        exit: None,
        by: Author::Person,
        started: Stamp(100),
        ended: None,
    }
}
fn done(exit: Option<i32>, last: u64, ended: u64) -> Block {
    Block {
        output: span(2, last),
        exit: exit.map(ExitCode),
        ended: Some(Stamp(ended)),
        ..running()
    }
}

#[test]
fn osc_133_marks_make_blocks() {
    let prompting = BlockState::Prompting { prompt: span(1, 1) };
    let typing = BlockState::Typing { prompt: span(1, 1) };
    let run = BlockState::Running(running());
    #[rustfmt::skip]
    let cases: Vec<(&str, BlockState, BlockIn, BlockState, Vec<BlockOut>)> = vec![
        ("A starts a prompt", BlockState::Idle, a(1), prompting.clone(), vec![BlockOut::PromptShown]),
        ("B starts typing", prompting.clone(), b(1), typing.clone(), vec![]),
        ("C runs the command", typing.clone(), c(2), run.clone(), vec![BlockOut::CommandStarted(BlockId(7))]),
        ("C without B still runs", prompting.clone(), c(2), run.clone(), vec![BlockOut::CommandStarted(BlockId(7))]),
        ("D finishes with its exit", run.clone(), d(5, Some(1)),
            BlockState::Finished(done(Some(1), 5, 200)), vec![BlockOut::BlockDone(done(Some(1), 5, 200))]),
        ("D without C ran nothing", typing.clone(), d(1, Some(130)), BlockState::Idle, vec![]),
        ("A while running ends the block unknown", run.clone(), a(9),
            BlockState::Prompting { prompt: span(9, 9) },
            vec![BlockOut::BlockDone(done(None, 2, 9)), BlockOut::PromptShown]),
        ("the shell exiting ends a running block", run.clone(), BlockIn::ShellExited { at: Stamp(300) },
            BlockState::Exited, vec![BlockOut::BlockDone(done(None, 2, 300)), BlockOut::ShellGone]),
        ("the shell exiting at a prompt", typing.clone(), BlockIn::ShellExited { at: Stamp(300) },
            BlockState::Exited, vec![BlockOut::ShellGone]),
        ("nothing after exit", BlockState::Exited, a(1), BlockState::Exited, vec![]),
        ("a stray D is ignored", BlockState::Idle, d(1, Some(0)), BlockState::Idle, vec![]),
        ("a stray C is ignored", BlockState::Idle, c(1), BlockState::Idle, vec![]),
    ];
    for (name, from, input, want, outs) in cases {
        let (state, out) = block_step(from, input);
        assert_eq!(state, want, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
    }
}
