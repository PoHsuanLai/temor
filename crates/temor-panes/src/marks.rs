//! The pane's commands, from the shell's marks (PLAN §9 blocks). Each OSC 133 mark arrives
//! numbered: its `MarkBegin`, its own event, then its `Anchor` with the same number. The rules are
//! `mark_step` in `temor-core`; here the terminal is read into its inputs (the clock, where the
//! anchor sits, the text typed) and its outputs are carried out (anchors handed back, a finished
//! command's line of output kept, and the command announced when it ran long).
//!
//! A screen restored from a snapshot keeps which cells were prompt, typed command and output,
//! but none of its marks reach the log as events. Its blocks are found again from those cells:
//! the rules are `found` in `temor-core`; here the terminal is read and anchored.

use std::time::Instant;

use temor_core::block::{Block, Note, Outcome, summarize};
use temor_core::session::found::{At, Kind, find_blocks, match_history};
use temor_core::session::{
    BlockAnchors, CommandRecord, KeptBlock, MarkIn, MarkKind, MarkOut, mark_step,
};
use temor_core::vocab::ids::{BlockId, ExitCode, GridRow, GridSpan};
use temor_vt::{
    AnchorId, ExitStatus, GridPoint, PointSpace, PromptEvent, PromptKind, SemanticMark,
};

use super::PaneCore;
use super::blocks::{anchor_id, handle};

/// A mark number no shell sends, for the prompt found on a restored screen.
const FOUND_SEQ: u64 = u64::MAX;

/// A finished command, as history is told of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub id: BlockId,
    pub command: String,
    pub exit: Option<i32>,
    pub ran_ms: u64,
}

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

impl PaneCore {
    /// A mark starts, numbered `seq`. Whatever an earlier mark left pending is dropped.
    pub fn mark_begin(&mut self, seq: u64) {
        self.mark(MarkIn::Begin { seq });
    }

    /// A mark's own event. Kept only while its mark is the current one.
    pub fn mark_event(&mut self, mark: PromptEvent) {
        self.mark(MarkIn::Event(kind_of(mark)));
    }

    /// Whether the mark in progress starts a command: `Some(true)` when the shell did not say
    /// which (the text the person sent names it), `Some(false)` when it did.
    pub fn starts_command(&self) -> Option<bool> {
        match self.blocks.marks.pending() {
            Some(MarkKind::OutputStart { command }) => Some(command.is_empty()),
            _ => None,
        }
    }

    /// A mark's anchor arrived, read by the pane at `read`. It is paired with the event of the
    /// same number; anything else is released, so a missed anchor cannot pair with the next
    /// mark's event. Returns whether a command finished.
    pub fn anchored(&mut self, seq: u64, id: AnchorId, read: Instant) -> bool {
        let finished_before = self.blocks.marks.finished();
        // The composer's text is the command as sent, lines and all, where the screen's rows
        // are only how the shell drew it; a command the shell names keeps its own name.
        let sent = match self.starts_command() {
            Some(unnamed) => self.sent.take().filter(|_| unnamed),
            None => None,
        };
        let row = self.screen_row(id).map(|row| GridRow(u64::from(row)));
        // When the pane read the mark, not when the window got to it: a command's start and end
        // taken in one go would otherwise be 0 ms apart.
        let at = self.clock.at(read);
        let typed = sent.or_else(|| self.typed_at(id));
        self.mark(MarkIn::Anchored {
            seq,
            anchor: handle(id),
            row,
            at,
            typed,
        });
        self.sweep();
        self.blocks.marks.finished() > finished_before
    }

    /// The command line as the person typed it, when the mark in progress starts the command
    /// without the shell saying what it is: the text from the input mark to where Enter was
    /// pressed, or to the output mark.
    fn typed_at(&self, output: AnchorId) -> Option<String> {
        if self.starts_command() != Some(true) {
            return None;
        }
        let marks = &self.blocks.marks;
        let from = anchor_id(marks.input()?);
        marks
            .enter()
            .and_then(|to| self.text_between(from, anchor_id(to)))
            .filter(|text| !text.is_empty())
            .or_else(|| self.text_between(from, output))
    }

    /// The shell is gone: a running command ends unreported.
    pub fn shell_gone(&mut self) {
        let at = self.clock.now();
        self.mark(MarkIn::ShellGone { at });
        self.sweep();
    }

    /// The terminal was reset: a running command is no longer on screen.
    pub fn terminal_reset(&mut self) {
        let at = self.clock.now();
        self.mark(MarkIn::Reset { at });
    }

    /// The person pressed Enter at the shell: the cursor now sits at the end of the typed command
    /// (a right prompt lies beyond it), so the command's text ends there.
    pub fn note_enter(&mut self) {
        let anchor = self.pane.with_term(|term| term.anchor_cursor());
        self.mark(MarkIn::Entered {
            anchor: anchor.map(handle),
        });
    }

    /// Drops the blocks whose marks the scrollback no longer holds.
    fn sweep(&mut self) {
        let ids = self.swept();
        for id in &ids {
            self.blocks.notes.remove(id);
        }
        if !ids.is_empty() {
            self.mark(MarkIn::Gone { ids });
        }
    }

    /// Steps the mark log on one input and carries out what it says: a command that finished
    /// keeps its card's line and may be announced.
    fn mark(&mut self, input: MarkIn) {
        let log = std::mem::take(&mut self.blocks.marks);
        let (log, outs) = mark_step(log, input);
        self.blocks.marks = log;
        for out in outs {
            match out {
                MarkOut::Release(anchor) => self.drop_anchor(anchor_id(anchor)),
                MarkOut::Finished(block) => {
                    self.keep_note(&block);
                    self.notify_finished(&block);
                }
                _ => {}
            }
        }
    }

    /// Keeps the line of output a finished block's card shows. Read once, when the block
    /// finishes: the card is drawn every frame and the output may be long.
    fn keep_note(&mut self, block: &Block) {
        let failed = matches!(block.outcome(), Outcome::Failed(_));
        let note = self
            .block_output(block.id)
            .and_then(|output| summarize(&output, failed));
        if let Some(note) = note {
            self.blocks.notes.insert(block.id, note);
        }
    }

    /// The row of an anchor in the screen (0 is the oldest kept row); `None` once it is gone.
    fn screen_row(&self, id: AnchorId) -> Option<u32> {
        self.pane
            .with_term(|term| term.anchor_point(id, PointSpace::Screen))
            .map(|at| at.row)
    }

    /// Hands an anchor back to the terminal.
    fn drop_anchor(&self, id: AnchorId) {
        self.pane.with_term(|term| term.release_anchor(id));
    }

    /// The text from one anchor up to (not including) another, trimmed of the line ends around
    /// it. The end cell is excluded: an anchor at a line's start marks the end of the line above.
    pub fn text_between(&self, from: AnchorId, to: AnchorId) -> Option<String> {
        self.pane.with_term(|term| {
            let start = term.anchor_point(from, PointSpace::Screen)?;
            let end = term.anchor_point(to, PointSpace::Screen)?;
            let last_col = term.size().cols.0.saturating_sub(1);
            let stop = match (end.col.0, end.row.checked_sub(1)) {
                (0, Some(row)) if end.row > start.row => {
                    GridPoint::new(PointSpace::Screen, last_col, row)
                }
                (col, _) if col > 0 && end.row >= start.row => {
                    GridPoint::new(PointSpace::Screen, col - 1, end.row)
                }
                _ => return Some(String::new()),
            };
            let text = term.text(start, stop).ok()?;
            Some(text.trim().to_owned())
        })
    }

    /// The blocks whose prompt and output the scrollback no longer holds.
    fn swept(&self) -> Vec<BlockId> {
        let pane = &self.pane;
        self.blocks
            .store()
            .iter()
            .map(|block| block.id)
            .filter(|&id| {
                !self.blocks.anchors_of(id).is_some_and(|held| {
                    [held.prompt, held.output]
                        .into_iter()
                        .flatten()
                        .any(|anchor| {
                            pane.with_term(|term| term.anchor_point(anchor, PointSpace::Screen))
                                .is_some()
                        })
                })
            })
            .collect()
    }

    /// The pane began as a kept screen: the commands it ran before are `history`, and the blocks
    /// on that screen are found again.
    pub fn begin_restored(&mut self, history: Vec<CommandRecord>) {
        self.inherited = history;
        self.find_restored_blocks();
    }

    /// Finds the blocks of a restored screen, when the log holds none still on it: each joins
    /// the log as a finished block, its status from what was kept of the pane's commands, and a
    /// prompt left open becomes the prompt being typed at.
    pub fn find_restored_blocks(&mut self) {
        self.sweep();
        if !self.blocks.marks.store().is_empty() {
            return;
        }
        let starts: Vec<(At, Kind)> = self
            .pane
            .with_term(|term| term.semantic_starts())
            .into_iter()
            .map(|(at, mark)| {
                let kind = match mark {
                    SemanticMark::Prompt => Kind::Prompt,
                    SemanticMark::Input => Kind::Input,
                    SemanticMark::Output => Kind::Output,
                };
                (
                    At {
                        row: u64::from(at.row),
                        col: at.col.0,
                    },
                    kind,
                )
            })
            .collect();
        let (found, typing) = find_blocks(&starts);
        let mut blocks = Vec::new();
        for block in &found {
            let anchors = (
                self.anchor_found(block.prompt),
                self.anchor_found(block.input),
                self.anchor_found(block.output),
            );
            let (Some(prompt), Some(input), Some(output)) = anchors else {
                let (a, b, c) = anchors;
                [a, b, c]
                    .into_iter()
                    .flatten()
                    .for_each(|id| self.drop_anchor(id));
                continue;
            };
            let end = block.end.and_then(|at| self.anchor_found(at));
            let command = self.text_between(input, output).unwrap_or_default();
            let last = block
                .end
                .map_or(block.output.row, |end| end.row.saturating_sub(1))
                .max(block.output.row);
            blocks.push(KeptBlock {
                anchors: BlockAnchors {
                    prompt: Some(handle(prompt)),
                    input: Some(handle(input)),
                    output: Some(handle(output)),
                    end: end.map(handle),
                },
                prompt: GridSpan {
                    first: GridRow(block.prompt.row),
                    last: GridRow(block.input.row),
                },
                output: GridSpan {
                    first: GridRow(block.output.row),
                    last: GridRow(last),
                },
                command,
                exit: None,
                took_ms: None,
            });
        }
        let commands: Vec<String> = blocks.iter().map(|b| b.command.clone()).collect();
        let (records, on_screen) = match_history(&commands, &self.inherited);
        for (block, record) in blocks.iter_mut().zip(records) {
            if let Some(record) = record {
                block.exit = record.exit.map(ExitCode);
                block.took_ms = record.took_ms;
            }
        }
        // The commands on the screen are its blocks now: kept once, not as history too.
        let keep = self.inherited.len() - on_screen;
        self.inherited.truncate(keep);
        let at = self.clock.now();
        if !blocks.is_empty() {
            self.mark(MarkIn::Restored { blocks, at });
            let restored: Vec<_> = self.blocks.marks.store().iter().cloned().collect();
            for block in &restored {
                self.keep_note(block);
            }
        }
        if let Some(typing) = typing {
            self.found_mark(MarkKind::PromptStart { primary: true }, typing.prompt);
            if let Some(input) = typing.input {
                self.found_mark(MarkKind::InputStart, input);
            }
        }
    }

    fn anchor_found(&self, at: At) -> Option<AnchorId> {
        let row = u32::try_from(at.row).ok()?;
        self.pane
            .with_term(|term| term.anchor_at(GridPoint::new(PointSpace::Screen, at.col, row)))
    }

    /// One mark the shell sent before the snapshot was taken, given to the log as if it came now.
    fn found_mark(&mut self, kind: MarkKind, at: At) {
        let Some(anchor) = self.anchor_found(at) else {
            return;
        };
        self.mark(MarkIn::Begin { seq: FOUND_SEQ });
        self.mark(MarkIn::Event(kind));
        let now = self.clock.now();
        self.mark(MarkIn::Anchored {
            seq: FOUND_SEQ,
            anchor: handle(anchor),
            row: Some(GridRow(at.row)),
            at: now,
            typed: None,
        });
    }

    /// How many rows the terminal has.
    pub fn rows(&self) -> u16 {
        self.pane.with_term(|term| term.size().rows.0)
    }

    /// The line of output a finished block's card shows, and how much output there was.
    pub fn block_note(&self, id: BlockId) -> Option<&Note> {
        self.blocks.notes.get(&id)
    }

    /// How many primary prompts the shell has marked so far.
    pub fn prompts_seen(&self) -> usize {
        self.blocks.marks.prompts()
    }

    /// Every block the pane holds, oldest first, with how each stands.
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

    /// The command a block ran, as typed.
    pub fn block_command(&self, id: BlockId) -> Option<&str> {
        self.blocks.store().get(id).map(|b| b.command.0.as_str())
    }

    /// A block's output as text, once the command has finished.
    pub fn block_output(&self, id: BlockId) -> Option<String> {
        let anchors = self.blocks.anchors_of(id)?;
        self.text_between(anchors.output?, anchors.end?)
    }
}
