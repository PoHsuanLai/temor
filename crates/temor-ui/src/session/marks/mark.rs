//! The events of one mark, in order: its number, its event, its anchor. A mark's anchor is kept
//! only when the block machine accepts the mark, so an ignored mark leaves nothing behind.

use ds_core::time::stamp::Stamp;
use temor_core::block::{Author, BlockIn, BlockOut, BlockState, block_step};
use temor_core::vocab::ids::{BlockId, ExitCode, GridRow};
use temor_core::vocab::text::CommandText;
use temor_vt::{AnchorId, ExitStatus, PromptEvent, PromptKind};

use super::super::Session;
use super::super::blocks::BlockAnchors;
use crate::session::Notice;

impl Session {
    /// A mark starts, numbered `seq`. Whatever an earlier mark left pending is dropped.
    pub(in crate::session) fn mark_begin(&mut self, seq: u64) {
        self.blocks.pending = None;
        self.blocks.pending_seq = Some(seq);
    }

    /// A mark's own event. Kept only while its mark is the current one.
    pub(in crate::session) fn mark_event(&mut self, mark: PromptEvent) {
        if self.blocks.pending_seq.is_some() {
            self.blocks.pending = Some(mark);
        }
    }

    /// A mark's anchor arrived. It is paired with the event of the same number; anything else is
    /// released, so a missed anchor cannot pair with the next mark's event.
    pub(in crate::session) fn anchored(&mut self, seq: u64, id: AnchorId) -> Notice {
        let current = self.blocks.pending_seq.take();
        let mark = self.blocks.pending.take();
        let (Some(current), Some(mark)) = (current, mark) else {
            self.drop_anchor(id);
            return Notice::Chrome;
        };
        if current != seq {
            self.drop_anchor(id);
            return Notice::Chrome;
        }
        let Some(row) = self.screen_row(id) else {
            self.drop_anchor(id);
            return Notice::Chrome;
        };
        let row = GridRow(u64::from(row));
        let at = self.clock.now();
        match mark {
            PromptEvent::PromptStart(PromptKind::Primary) => {
                if self.step(BlockIn::PromptStart { row, at }) {
                    self.blocks.prompts += 1;
                    if let Some(old) = self.blocks.prompt.replace(id) {
                        self.drop_anchor(old);
                    }
                    if let Some(old) = self.blocks.input.take() {
                        self.drop_anchor(old);
                    }
                    self.clear_enter();
                } else {
                    self.drop_anchor(id);
                }
            }
            PromptEvent::PromptStart(_) => self.drop_anchor(id),
            PromptEvent::InputStart => {
                if self.step(BlockIn::CommandStart { row }) {
                    if let Some(old) = self.blocks.input.replace(id) {
                        self.drop_anchor(old);
                    }
                } else {
                    self.drop_anchor(id);
                }
            }
            PromptEvent::OutputStart { command } => self.output_start(id, row, at, &command),
            PromptEvent::CommandEnd { exit, .. } => {
                let running = match &self.blocks.machine {
                    BlockState::Running(block) => Some(block.id),
                    _ => None,
                };
                let exit = match exit {
                    ExitStatus::Code(code) => Some(ExitCode(code.0)),
                    // Unreported, or a status this build does not know: no exit code.
                    _ => None,
                };
                let taken = self.step(BlockIn::Finished { row, exit, at });
                match (taken, running) {
                    (true, Some(block)) => {
                        self.blocks.anchors.entry(block).or_default().end = Some(id)
                    }
                    _ => self.drop_anchor(id),
                }
            }
            // A prompt event this build does not know: no block is opened or closed for it.
            _ => self.drop_anchor(id),
        }
        self.sweep();
        Notice::Chrome
    }

    /// The command started: its block keeps the prompt, input and output anchors, if accepted.
    fn output_start(&mut self, id: AnchorId, row: GridRow, at: Stamp, command: &str) {
        let input = self.blocks.input.take();
        let enter = self.blocks.enter.take();
        let typed = input.and_then(|from| {
            enter
                .and_then(|to| self.text_between(from, to))
                .filter(|text| !text.is_empty())
                .or_else(|| self.text_between(from, id))
        });
        if let Some(enter) = enter {
            self.drop_anchor(enter);
        }
        let text = match command.is_empty() {
            true => typed.unwrap_or_default(),
            false => command.to_owned(),
        };
        let block = BlockId(self.blocks.next_id);
        let accepted = self.step(BlockIn::OutputStart {
            id: block,
            row,
            command: CommandText::from_shell(&text),
            by: Author::Person,
            at,
        });
        if accepted {
            self.blocks.next_id += 1;
            let anchors = BlockAnchors {
                prompt: self.blocks.prompt.take(),
                input: None,
                output: Some(id),
                end: None,
            };
            self.blocks.anchors.insert(block, anchors);
        } else {
            // An ignored command keeps nothing: its anchors go back to the terminal.
            self.drop_anchor(id);
            if let Some(prompt) = self.blocks.prompt.take() {
                self.drop_anchor(prompt);
            }
            if let Some(input) = input {
                self.drop_anchor(input);
            }
        }
    }

    /// The shell is gone: a running command ends unreported.
    pub(in crate::session) fn shell_gone(&mut self) {
        let at = self.clock.now();
        self.step(BlockIn::ShellExited { at });
        self.sweep();
    }

    /// The terminal was reset: a running command is no longer on screen.
    pub(in crate::session) fn terminal_reset(&mut self) {
        let at = self.clock.now();
        self.step(BlockIn::Reset { at });
    }

    /// The person pressed Enter at the shell: the cursor now sits at the end of the typed command
    /// (a right prompt lies beyond it), so the command's text ends there.
    pub(in crate::session) fn note_enter(&mut self) {
        self.clear_enter();
        self.blocks.enter = self.pane.with_term(|term| term.anchor_cursor());
    }

    fn clear_enter(&mut self) {
        if let Some(old) = self.blocks.enter.take() {
            self.drop_anchor(old);
        }
    }

    /// Runs the block machine on one input and applies what it says. Returns whether the machine
    /// took the input (changed state): an ignored input leaves nothing to keep.
    fn step(&mut self, input: BlockIn) -> bool {
        let machine = std::mem::replace(&mut self.blocks.machine, BlockState::Idle);
        let before = machine.clone();
        let (next, outs) = block_step(machine, input);
        let taken = next != before;
        self.blocks.machine = next;
        for out in &outs {
            if let BlockOut::BlockDone(block) = out {
                self.notify_finished(block);
            }
            let gone = self.blocks.store.apply(out);
            for anchor in self.blocks.drop_anchors(&gone) {
                self.drop_anchor(anchor);
            }
        }
        taken
    }
}
