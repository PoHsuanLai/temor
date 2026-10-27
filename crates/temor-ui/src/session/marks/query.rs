//! What the bar and the prompt jumps read from the blocks, and what a jump does to the view.
//! Nothing here is drawn; the views decide how to show it.

use temor_core::block::Outcome;
use temor_core::session::AnnotateIn;
use temor_core::vocab::ids::BlockId;

use super::super::Session;

/// A finished command, as history is told of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub id: BlockId,
    pub command: String,
    pub exit: Option<i32>,
    pub ran_ms: u64,
}

impl Session {
    /// How many rows the terminal has.
    pub fn rows(&self) -> u16 {
        self.pane.with_term(|term| term.size().rows.0)
    }

    /// Keeps the line of output a finished block's card shows. Read once, when the block
    /// finishes: the card is drawn every frame and the output may be long.
    pub(super) fn keep_note(&mut self, block: &temor_core::block::Block) {
        let failed = matches!(block.outcome(), Outcome::Failed(_));
        let note = self
            .block_output(block.id)
            .and_then(|output| temor_core::block::summarize(&output, failed));
        if let Some(note) = note {
            self.blocks.notes.insert(block.id, note);
        }
    }

    /// The line of output a finished block's card shows, and how much output there was.
    pub fn block_note(&self, id: BlockId) -> Option<&temor_core::block::Note> {
        self.blocks.notes.get(&id)
    }

    /// How many primary prompts the shell has marked so far.
    pub fn prompts_seen(&self) -> usize {
        self.blocks.marks.prompts()
    }

    /// Every block the session holds, oldest first, with how each stands.
    pub fn block_outcomes(&self) -> Vec<(BlockId, Outcome)> {
        self.blocks
            .store()
            .iter()
            .map(|block| (block.id, block.outcome()))
            .collect()
    }

    /// The finished blocks after `after` (every one when `None`), oldest first, as history keeps
    /// them: the command, how it exited, and how long it ran.
    pub fn finished_after(&self, after: Option<BlockId>) -> Vec<Finished> {
        self.blocks
            .store()
            .iter()
            .filter(|block| after.is_none_or(|after| block.id > after))
            .filter_map(|block| {
                Some(Finished {
                    id: block.id,
                    command: block.command.0.clone(),
                    exit: block.exit.map(|code| code.0),
                    ran_ms: block.elapsed_ms()?,
                })
            })
            .collect()
    }

    /// Whether a block's command has finished (its output is complete).
    pub fn block_finished(&self, id: BlockId) -> bool {
        self.blocks
            .store()
            .get(id)
            .is_some_and(|block| block.outcome() != Outcome::Running)
    }

    /// Picks no block.
    pub fn clear_block_selection(&mut self) {
        self.annotate(AnnotateIn::ClearPicks);
    }

    /// The picked block the arrows move (the last one clicked), if any.
    pub fn selected_block(&self) -> Option<BlockId> {
        self.ann.active_pick()
    }

    /// The command a block ran, as typed.
    pub fn block_command(&self, id: BlockId) -> Option<&str> {
        self.blocks.store().get(id).map(|b| b.command.0.as_str())
    }

    /// A block's output as text, once the command has finished.
    pub fn block_output(&self, id: BlockId) -> Option<String> {
        let anchors = self.blocks.anchors_of(id)?;
        self.text_between(anchors.output?, anchors.end?)
    }

    /// The output of the block that holds screen row `row`, once its command has finished.
    pub fn output_at_row(&self, row: u32) -> Option<String> {
        self.frame()
            .and_then(|frame| frame.block_at_screen_row(row))
            .and_then(|id| self.block_output(id))
    }
}
