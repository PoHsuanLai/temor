//! The pane's marks as the window hears them. The commands are the pane's (`PaneCore` in
//! `temor-client`); here the window adds what only it has (the text the composer sent) and
//! announces a command that finished.

use std::time::Instant;

use temor_core::block::Block;
use temor_vt::{AnchorId, PromptEvent};

use super::super::Session;
use crate::session::Notice;

impl Session {
    /// A mark starts, numbered `seq`.
    pub(in crate::session) fn mark_begin(&mut self, seq: u64) {
        let finished = self.core.mark_begin(seq);
        self.announce_finished(&finished);
    }

    /// A mark's own event.
    pub(in crate::session) fn mark_event(&mut self, mark: PromptEvent) {
        let finished = self.core.mark_event(mark);
        self.announce_finished(&finished);
    }

    /// A mark's anchor arrived, read by the pane at `read`.
    pub(in crate::session) fn anchored(&mut self, seq: u64, id: AnchorId, read: Instant) -> Notice {
        let finished_before = self.core.blocks.marks.finished();
        // The composer's text is the command as sent, lines and all; a command the shell names
        // keeps its own name.
        let sent = match self.core.starts_command() {
            Some(unnamed) => self.take_composer_sent().filter(|_| unnamed),
            None => None,
        };
        let finished = self.core.anchored(seq, id, read, sent);
        self.announce_finished(&finished);
        match self.core.blocks.marks.finished() > finished_before {
            true => Notice::CommandDone,
            false => Notice::Chrome,
        }
    }

    /// The shell is gone: a running command ends unreported.
    pub(in crate::session) fn shell_gone(&mut self) {
        let finished = self.core.shell_gone();
        self.announce_finished(&finished);
    }

    /// The terminal was reset: a running command is no longer on screen.
    pub(in crate::session) fn terminal_reset(&mut self) {
        let finished = self.core.terminal_reset();
        self.announce_finished(&finished);
    }

    /// The person pressed Enter at the shell.
    pub(in crate::session) fn note_enter(&mut self) {
        let finished = self.core.note_enter();
        self.announce_finished(&finished);
    }

    /// The pane began as a kept screen: the commands it ran before are `history`, and the blocks
    /// on that screen are found again.
    pub fn begin_restored(&mut self, history: Vec<temor_core::session::CommandRecord>) {
        let finished = self.core.begin_restored(history);
        self.announce_finished(&finished);
    }

    /// Finds the blocks of a restored screen again.
    pub(in crate::session) fn find_restored_blocks(&mut self) {
        let finished = self.core.find_restored_blocks();
        self.announce_finished(&finished);
    }

    fn announce_finished(&mut self, finished: &[Block]) {
        for block in finished {
            self.notify_finished(block);
        }
    }
}
