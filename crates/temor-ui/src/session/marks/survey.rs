//! Reads the terminal into a [`Frame`]: the one place the blocks' marks are asked for their rows.
//! Everything the views want to know about where the blocks are comes from the frame this makes.

use temor_core::block::BlockState;
use temor_core::session::frame::{BlockRead, End, Frame, Lead, LiveRead, Read, Spot};
use temor_pane::Term;
use temor_vt::{AnchorId, PointSpace};

use super::super::Session;
use super::bands::lead_at;
use super::query::used_columns;

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
            *self.blocks.machine(),
            BlockState::Prompting { .. } | BlockState::Typing { .. }
        );
        let painted = self.painter.cursor_cell().map(|cell| u32::from(cell.row.0));
        let first_shown = self
            .layout
            .as_ref()
            .map_or(0, |layout| u32::from(layout.visible_rows().start));
        self.pane.with_term(|term| {
            if term.on_alternate_screen() {
                return None;
            }
            let (top, len) = term.viewport().ok()?;
            // Spans and tools keep clear of the cursor only where it is drawn; the bands' running
            // command runs to this frame's cursor row when the terminal knows it.
            let cursor = painted.map(|row| top.saturating_add(row));
            let live_cursor = term
                .cursor_viewport_row()
                .map(u32::from)
                .or(painted)
                .map(|row| top.saturating_add(row));
            let blocks = self
                .blocks
                .store()
                .iter()
                .map(|block| {
                    let anchors = self.blocks.anchors_of(block.id).unwrap_or_default();
                    let prompt = spot(term, anchors.prompt);
                    let lead = match anchors.prompt {
                        Some(prompt) => lead_at(term, prompt, anchors.input),
                        None => Lead::Header,
                    };
                    // The header line's width is wanted only where a mark is drawn beside it.
                    let header_cols = match prompt {
                        Some(at) if (top..top.saturating_add(len)).contains(&at.row) => {
                            let below = u32::from(lead == Lead::Blank);
                            term.row_text(at.row + below)
                                .map_or(0, |text| used_columns(&text.cells))
                        }
                        _ => 0,
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
                        header_cols,
                        picked: self.ann.is_picked(block.id),
                        bookmarked: self.ann.is_bookmarked(block.id),
                    }
                })
                .collect();
            let live = self.blocks.prompt().filter(|_| typing).and_then(|prompt| {
                let prompt = spot(term, Some(prompt))?;
                let input = spot(term, self.blocks.input());
                // The header's line, as `Frame::live_header` finds it.
                let header_row = match input {
                    Some(at) if at.row > prompt.row && at.col <= 1 => at.row - 1,
                    Some(at) => at.row,
                    None => prompt.row,
                };
                Some(LiveRead {
                    prompt,
                    lead: lead_at(term, self.blocks.prompt()?, self.blocks.input()),
                    input,
                    header_text: term
                        .row_text(header_row)
                        .map(|text| text.text().trim_end().to_owned())
                        .unwrap_or_default(),
                })
            });
            Some(Read {
                top,
                len,
                cursor,
                live_cursor,
                first_shown,
                blocks,
                live,
            })
        })
    }
}
