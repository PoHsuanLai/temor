//! Reads the terminal into a [`Frame`]: the one place the blocks' marks are asked for their rows.
//! Everything the views want to know about where the blocks are comes from the frame this makes.

use temor_core::block::BlockState;
use temor_core::session::frame::{BlockRead, End, Frame, Lead, LiveRead, Read, Spot};
use temor_pane::Term;
use temor_vt::{AnchorId, PointSpace};

use super::super::Session;
use super::lead::lead_at;

/// Where an anchor is in the screen.
fn spot(term: &Term, anchor: Option<AnchorId>) -> Option<Spot> {
    let at = term.anchor_point(anchor?, PointSpace::Screen)?;
    Some(Spot {
        row: at.row,
        col: at.col.0,
    })
}

impl Session {
    /// The blocks as they sit in the viewport now. `None` while a full-screen program holds the
    /// screen (the shell's blocks are not shown there), or before the terminal has a viewport.
    pub fn frame(&self) -> Option<Frame> {
        self.survey().map(Frame::of)
    }

    pub(crate) fn survey(&self) -> Option<Read> {
        let typing = matches!(
            *self.core.blocks.machine(),
            BlockState::Prompting { .. } | BlockState::Typing { .. }
        );
        let painted = self.painter.cursor_cell().map(|cell| u32::from(cell.row.0));
        self.core.pane.with_term(|term| {
            if term.on_alternate_screen() {
                return None;
            }
            let (top, len) = term.viewport().ok()?;
            // A running block ends at the cursor only where it is drawn: a program that hides it
            // draws below it too.
            let cursor = painted.map(|row| top.saturating_add(row));
            let blocks = self
                .core
                .blocks
                .store()
                .iter()
                .map(|block| {
                    let anchors = self.core.blocks.anchors_of(block.id).unwrap_or_default();
                    let prompt = spot(term, anchors.prompt);
                    let lead = match anchors.prompt {
                        Some(prompt) => lead_at(term, prompt, anchors.input),
                        None => Lead::Header,
                    };
                    let end = match (anchors.end, spot(term, anchors.end)) {
                        (None, _) => End::Running,
                        (Some(_), Some(at)) => End::At(at),
                        (Some(_), None) => End::Lost,
                    };
                    BlockRead {
                        id: block.id,
                        command: block.command.0.clone(),
                        outcome: block.outcome(),
                        took_ms: block.elapsed_ms(),
                        prompt,
                        prompt_lost: anchors.prompt.is_some() && prompt.is_none(),
                        output: spot(term, anchors.output),
                        input: spot(term, anchors.input),
                        end,
                        lead,
                        picked: self.ann.is_picked(block.id),
                    }
                })
                .collect();
            let live = self
                .core
                .blocks
                .prompt()
                .filter(|_| typing)
                .and_then(|prompt| {
                    Some(LiveRead {
                        prompt: spot(term, Some(prompt))?,
                    })
                });
            Some(Read {
                top,
                len,
                cursor,
                blocks,
                live,
            })
        })
    }
}
