//! The events of one mark, in order: its number, its event, its anchor. The rules are
//! `mark_step` in `temor-core`; here the terminal is read into its inputs (the clock, where the
//! anchor sits, the text typed) and its outputs are carried out (anchors handed back, a finished
//! command announced).

use temor_core::session::{MarkIn, MarkKind, MarkOut, mark_step};
use temor_core::vocab::ids::{ExitCode, GridRow};
use temor_vt::{AnchorId, ExitStatus, PromptEvent, PromptKind};

use super::super::Session;
use super::super::blocks::{anchor_id, handle};
use crate::session::Notice;

/// What a mark's event says, for the log.
fn kind_of(event: PromptEvent) -> MarkKind {
    match event {
        PromptEvent::PromptStart(kind) => MarkKind::PromptStart {
            primary: kind == PromptKind::Primary,
        },
        PromptEvent::InputStart => MarkKind::InputStart,
        PromptEvent::OutputStart { command } => MarkKind::OutputStart { command },
        PromptEvent::CommandEnd { exit, .. } => MarkKind::CommandEnd {
            exit: match exit {
                ExitStatus::Code(code) => Some(ExitCode(code.0)),
                // Unreported, or a status this build does not know: no exit code.
                _ => None,
            },
        },
        // A prompt event this build does not know: no block is opened or closed for it.
        _ => MarkKind::Other,
    }
}

impl Session {
    /// A mark starts, numbered `seq`. Whatever an earlier mark left pending is dropped.
    pub(in crate::session) fn mark_begin(&mut self, seq: u64) {
        self.mark(MarkIn::Begin { seq });
    }

    /// A mark's own event. Kept only while its mark is the current one.
    pub(in crate::session) fn mark_event(&mut self, mark: PromptEvent) {
        self.mark(MarkIn::Event(kind_of(mark)));
    }

    /// A mark's anchor arrived. It is paired with the event of the same number; anything else is
    /// released, so a missed anchor cannot pair with the next mark's event.
    pub(in crate::session) fn anchored(&mut self, seq: u64, id: AnchorId) -> Notice {
        let finished_before = self.blocks.marks.finished();
        let row = self.screen_row(id).map(|row| GridRow(u64::from(row)));
        let at = self.clock.now();
        let typed = self.typed_at(id);
        self.mark(MarkIn::Anchored {
            seq,
            anchor: handle(id),
            row,
            at,
            typed,
        });
        self.sweep();
        match self.blocks.marks.finished() > finished_before {
            true => Notice::CommandDone,
            false => Notice::Chrome,
        }
    }

    /// The command line as the person typed it, when the mark in progress starts the command
    /// without the shell saying what it is: the text from the input mark to where Enter was
    /// pressed, or to the output mark.
    fn typed_at(&self, output: AnchorId) -> Option<String> {
        let marks = &self.blocks.marks;
        if !matches!(marks.pending(), Some(MarkKind::OutputStart { command }) if command.is_empty())
        {
            return None;
        }
        let from = anchor_id(marks.input()?);
        marks
            .enter()
            .and_then(|to| self.text_between(from, anchor_id(to)))
            .filter(|text| !text.is_empty())
            .or_else(|| self.text_between(from, output))
    }

    /// The shell is gone: a running command ends unreported.
    pub(in crate::session) fn shell_gone(&mut self) {
        let at = self.clock.now();
        self.mark(MarkIn::ShellGone { at });
        self.sweep();
    }

    /// The terminal was reset: a running command is no longer on screen.
    pub(in crate::session) fn terminal_reset(&mut self) {
        let at = self.clock.now();
        self.mark(MarkIn::Reset { at });
    }

    /// The person pressed Enter at the shell: the cursor now sits at the end of the typed command
    /// (a right prompt lies beyond it), so the command's text ends there.
    pub(in crate::session) fn note_enter(&mut self) {
        let anchor = self.pane.with_term(|term| term.anchor_cursor());
        self.mark(MarkIn::Entered {
            anchor: anchor.map(handle),
        });
    }

    /// Drops the blocks whose marks the scrollback no longer holds.
    pub(super) fn sweep(&mut self) {
        let ids = self.swept();
        if !ids.is_empty() {
            self.mark(MarkIn::Gone { ids });
        }
    }

    /// Steps the mark log on one input and carries out what it says.
    pub(super) fn mark(&mut self, input: MarkIn) {
        let log = std::mem::take(&mut self.blocks.marks);
        let (log, outs) = mark_step(log, input);
        self.blocks.marks = log;
        for out in outs {
            match out {
                MarkOut::Release(anchor) => self.drop_anchor(anchor_id(anchor)),
                MarkOut::Finished(block) => self.notify_finished(&block),
                _ => {}
            }
        }
    }
}
