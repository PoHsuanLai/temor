//! What the bar and the prompt jumps read from the blocks, and what a jump does to the view.
//! Nothing here is drawn; the views decide how to show it.

use temor_client::model::block::Outcome;
use temor_client::model::vocab::ids::BlockId;

use super::super::Session;
use super::Finished;

impl Session {
    /// How many rows the terminal has.
    pub fn rows(&self) -> u16 {
        self.core().rows()
    }

    /// The line of output a finished block's card shows, and how much output there was.
    pub fn block_note(&self, id: BlockId) -> Option<temor_client::model::block::Note> {
        self.core().block_note(id).cloned()
    }

    /// How many primary prompts the shell has marked so far.
    pub fn prompts_seen(&self) -> usize {
        self.core().prompts_seen()
    }

    /// Every block the session holds, oldest first, with how each stands.
    pub fn block_outcomes(&self) -> Vec<(BlockId, Outcome)> {
        self.core().block_outcomes()
    }

    /// The finished blocks after `after` (every one when `None`), oldest first, as history keeps
    /// them: the command, how it exited, and how long it ran.
    pub fn finished_after(&self, after: Option<BlockId>) -> Vec<Finished> {
        self.core().finished_after(after)
    }

    /// Whether a block's command has finished (its output is complete).
    pub fn block_finished(&self, id: BlockId) -> bool {
        self.core().block_finished(id)
    }

    /// The picked block the arrows move (the last one clicked), if any.
    pub fn selected_block(&self) -> Option<BlockId> {
        self.ann.active_pick()
    }

    /// The command a block ran, as typed.
    pub fn block_command(&self, id: BlockId) -> Option<String> {
        self.core().block_command(id).map(str::to_owned)
    }

    /// A block's output as text, once the command has finished.
    pub fn block_output(&self, id: BlockId) -> Option<String> {
        self.core().block_output(id)
    }

    /// The output of the block that holds screen row `row`, once its command has finished.
    pub fn output_at_row(&self, row: u32) -> Option<String> {
        self.frame()
            .and_then(|frame| frame.block_at_screen_row(row))
            .and_then(|id| self.block_output(id))
    }
}
