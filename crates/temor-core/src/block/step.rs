//! `block_step`: one shell's OSC 133 marks turned into blocks.
//!
//! A prompt starts (A), the person types (B), the command runs (C), it finishes (D). Shells skip
//! marks: a missing B is read as typing that began with the prompt, a new A while a command runs
//! ends that command with no exit status, and a D with no C (an empty line, or Ctrl-C at the
//! prompt) ran nothing and makes no block.

use super::model::{Author, Block};
use crate::vocab::ids::{BlockId, ExitCode, GridRow, GridSpan};
use crate::vocab::text::CommandText;
use ds_core::time::stamp::Stamp;

/// Where a shell stands between prompts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockState {
    /// No prompt seen yet.
    Idle,
    /// The prompt is being drawn.
    Prompting {
        /// Its rows so far.
        prompt: GridSpan,
    },
    /// The person is at the prompt.
    Typing {
        /// The prompt's rows.
        prompt: GridSpan,
    },
    /// A command is running.
    Running(Block),
    /// The last command finished.
    Finished(Block),
    /// The shell is gone.
    Exited,
}

/// A mark from the shell, or its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockIn {
    /// OSC 133 A: a prompt starts at `row`.
    PromptStart {
        /// Where.
        row: GridRow,
        /// When.
        at: Stamp,
    },
    /// OSC 133 B: the prompt ends and typing starts at `row`.
    CommandStart {
        /// Where.
        row: GridRow,
    },
    /// OSC 133 C: the command line was accepted and output starts at `row`.
    OutputStart {
        /// The new block's id, which the caller allocates.
        id: BlockId,
        /// Where output starts.
        row: GridRow,
        /// The command line.
        command: CommandText,
        /// Who wrote it.
        by: Author,
        /// When.
        at: Stamp,
    },
    /// OSC 133 D: the command finished at `row`.
    Finished {
        /// The last row of output.
        row: GridRow,
        /// Its exit status, if the shell sent one.
        exit: Option<ExitCode>,
        /// When.
        at: Stamp,
    },
    /// The shell process exited.
    ShellExited {
        /// When.
        at: Stamp,
    },
    /// The terminal was reset (RIS): a running command is no longer on screen, so it ends with
    /// no exit status.
    Reset {
        /// When.
        at: Stamp,
    },
}

/// What follows from a mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockOut {
    /// A new prompt is up: presence clears Working and Waiting, a typed proposal is gone.
    PromptShown,
    /// The shell left the prompt to run this command. The block is in its running state.
    CommandStarted(Block),
    /// A block is complete.
    BlockDone(Block),
    /// The shell is gone.
    ShellGone,
}

/// The shell after `input`.
pub fn block_step(state: BlockState, input: BlockIn) -> (BlockState, Vec<BlockOut>) {
    use BlockState as S;
    match (state, input) {
        (S::Exited, _) => (S::Exited, vec![]),
        (state, BlockIn::ShellExited { at }) => {
            let mut out = ended(state, None, at);
            out.push(BlockOut::ShellGone);
            (S::Exited, out)
        }
        (state, BlockIn::Reset { at }) => (S::Idle, ended(state, None, at)),
        (state, BlockIn::PromptStart { row, at }) => {
            let mut out = ended(state, None, at);
            out.push(BlockOut::PromptShown);
            (
                S::Prompting {
                    prompt: GridSpan::row(row),
                },
                out,
            )
        }
        (S::Prompting { prompt }, BlockIn::CommandStart { row }) => (
            S::Typing {
                prompt: prompt.reaching(row),
            },
            vec![],
        ),
        (
            S::Prompting { prompt } | S::Typing { prompt },
            BlockIn::OutputStart {
                id,
                row,
                command,
                by,
                at,
            },
        ) => {
            let block = Block {
                id,
                prompt,
                command,
                output: GridSpan::row(row),
                exit: None,
                by,
                started: at,
                ended: None,
            };
            let running = block.clone();
            (S::Running(block), vec![BlockOut::CommandStarted(running)])
        }
        (S::Typing { .. } | S::Prompting { .. }, BlockIn::Finished { .. }) => (S::Idle, vec![]),
        (S::Running(block), BlockIn::Finished { row, exit, at }) => {
            let done = Block {
                output: block.output.reaching(row),
                exit,
                ended: Some(at),
                ..block
            };
            (S::Finished(done.clone()), vec![BlockOut::BlockDone(done)])
        }
        (state, _) => (state, vec![]),
    }
}

/// A running block ended by something other than its D mark, with no exit status.
fn ended(state: BlockState, exit: Option<ExitCode>, at: Stamp) -> Vec<BlockOut> {
    match state {
        BlockState::Running(block) => vec![BlockOut::BlockDone(Block {
            exit,
            ended: Some(at),
            ..block
        })],
        _ => vec![],
    }
}
