//! A screen restored from a snapshot keeps which cells were prompt, typed command and output,
//! but none of its marks reach the log as events. Its blocks are found again from those cells:
//! the rules are `found` in `temor-core`; here the terminal is read and anchored.

use temor_core::session::found::{At, Kind, find_blocks, match_history};
use temor_core::session::{BlockAnchors, KeptBlock, MarkIn, MarkKind};
use temor_core::vocab::ids::{ExitCode, GridRow, GridSpan};
use temor_vt::{AnchorId, GridPoint, PointSpace, SemanticMark};

use super::super::Session;
use super::super::blocks::handle;

/// A mark number no shell sends, for the prompt found on a restored screen.
const FOUND_SEQ: u64 = u64::MAX;

impl Session {
    /// The pane began as a kept screen: the commands it ran before are `history`, and the blocks
    /// on that screen are found again.
    pub fn begin_restored(&mut self, history: Vec<temor_core::session::CommandRecord>) {
        self.inherited = history;
        self.find_restored_blocks();
    }

    /// Finds the blocks of a restored screen, when the log holds none still on it: each joins
    /// the log as a finished block, its status from what was kept of the pane's commands, and a
    /// prompt left open becomes the prompt being typed at.
    pub(in crate::session) fn find_restored_blocks(&mut self) {
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
}
