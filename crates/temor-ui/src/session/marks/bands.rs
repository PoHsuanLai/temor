//! The command cards as row bands for the renderer: a rule above every block (the line being
//! typed is set off from history by the rule above its block, with no fill of its own), a
//! stripe and a faint fill on a failed one, an accent fill on the selected one, and the command
//! line in bold. temor's prompt leaves a blank line above its header; the card's edge runs
//! across the middle of that line, so every card has half a line of room above its header and
//! half a line below its last row, as Warp's blocks do. `bands_of` is pure;
//! `Session::card_bands` gathers its input from the terminal.

use temor_core::block::{BlockState, Outcome};
use temor_pane::Term;
use temor_pane::text::RowText;
use temor_render::{Cut, RowBand, RowSpan, RowWeight};
use temor_vt::{AnchorId, PointSpace, Row};

use super::super::Session;
use crate::view::card_colours::CardColours;

/// Where a block stands, as far as its card goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    /// The prompt is up and the command has not started.
    AtPrompt,
    /// The command started (or finished) with this outcome.
    Ran(Outcome),
}

/// Whether the block is the one the gutter selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pick {
    Selected,
    Unselected,
}

/// Whether a block's first row is the blank line temor's prompt leaves above its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lead {
    Blank,
    Header,
}

/// One block: its first row in the viewport (negative above it), what that row is, how it
/// stands, and the rows of its command line (OSC 133 B up to C), when they are known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CardBlock {
    pub first: i64,
    pub lead: Lead,
    pub stage: Stage,
    pub pick: Pick,
    pub command: Option<(i64, i64)>,
}

/// Whether a prompt's first row is a blank line: nothing but spaces, and a row below it that the
/// prompt goes on to (its header).
pub(super) fn leads_blank(first: &RowText, input_row: Option<i64>, first_row: i64) -> Lead {
    let blank = first.cells.iter().all(|cell| cell.text.trim().is_empty());
    match blank && input_row.is_some_and(|input| input > first_row) {
        true => Lead::Blank,
        false => Lead::Header,
    }
}

/// What a prompt's first row is in the terminal, from its prompt and input marks; a header when
/// the marks or the row can't be read.
pub(super) fn lead_at(term: &Term, prompt: AnchorId, input: Option<AnchorId>) -> Lead {
    let row_of = |id| term.anchor_point(id, PointSpace::Screen).map(|p| p.row);
    let Some(first) = row_of(prompt) else {
        return Lead::Header;
    };
    let input = input.and_then(row_of).map(i64::from);
    term.row_text(first).map_or(Lead::Header, |text| {
        leads_blank(&text, input, i64::from(first))
    })
}

/// The bands for `blocks` (oldest first) on a viewport of `rows`, the last block running to
/// viewport row `end`. Each block runs to the row before the next one's first row, or across the
/// middle of that row when it is blank; a block that begins on a blank row begins across its
/// middle.
pub(super) fn bands_of(
    blocks: &[CardBlock],
    end: i64,
    rows: u16,
    colours: &CardColours,
) -> Vec<RowBand> {
    let mut out = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        let (last, bottom_cut) = match blocks.get(i + 1) {
            Some(next) if next.lead == Lead::Blank => (next.first, Cut::Middle),
            Some(next) => (next.first - 1, Cut::Edge),
            None => (end, Cut::Edge),
        };
        let last = last.max(block.first);
        let top = block.first.max(0);
        let bottom = last.min(i64::from(rows) - 1);
        if top > bottom {
            continue;
        }
        // A cut row out of view is clipped to the view's edge.
        let top_cut = match (block.lead, block.first >= 0) {
            (Lead::Blank, true) => Cut::Middle,
            _ => Cut::Edge,
        };
        let bottom_cut = match last == bottom {
            true => bottom_cut,
            false => Cut::Edge,
        };
        let fill = match (block.pick, block.stage) {
            (Pick::Selected, _) => Some(colours.selected_fill),
            (Pick::Unselected, Stage::Ran(Outcome::Failed(_))) => Some(colours.failed_fill),
            (Pick::Unselected, _) => None,
        };
        let stripe = match block.stage {
            Stage::Ran(Outcome::Failed(_)) => Some(colours.failed_stripe),
            _ => None,
        };
        // A block whose first row is row 0 (or above the view) shows no rule, unless the rule
        // crosses that row's middle.
        let rule = (block.first >= 1 || top_cut == Cut::Middle).then_some(colours.rule);
        if !(fill.is_none() && stripe.is_none() && rule.is_none()) {
            out.push(RowBand {
                rows: RowSpan {
                    first: Row(top as u16),
                    last: Row(bottom as u16),
                },
                top: top_cut,
                bottom: bottom_cut,
                fill,
                stripe,
                rule,
                weight: RowWeight::Normal,
            });
        }
        // The command line draws bold, clipped to the view.
        if let Some((from, to)) = block.command {
            let (from, to) = (from.max(0), to.min(i64::from(rows) - 1));
            if from <= to {
                out.push(RowBand {
                    rows: RowSpan {
                        first: Row(from as u16),
                        last: Row(to as u16),
                    },
                    top: Cut::Edge,
                    bottom: Cut::Edge,
                    fill: None,
                    stripe: None,
                    rule: None,
                    weight: RowWeight::Bold,
                });
            }
        }
    }
    out
}

/// The rows of a command line: from the row of the input mark to the row before the output
/// mark when that mark is at a line's start, else to the output mark's row. `output` is the
/// mark's row and column; without it the line runs to `fallback_end`, never above `input`.
pub(super) fn command_rows(
    input: i64,
    output: Option<(i64, u16)>,
    fallback_end: i64,
) -> (i64, i64) {
    let last = match output {
        Some((row, 0)) if row > input => row - 1,
        Some((row, _)) => row,
        None => fallback_end,
    };
    (input, last.max(input))
}

impl Session {
    /// The viewport row of the header line of the prompt being typed at: with temor's prompt,
    /// the line above the empty typing line; with a one-line prompt, the prompt's own line.
    /// None while a command runs, on the alternate screen, or when it is out of view.
    pub fn live_prompt_row(&self) -> Option<u32> {
        let typing = matches!(
            self.blocks.machine,
            BlockState::Prompting { .. } | BlockState::Typing { .. }
        );
        let prompt = self.blocks.prompt.filter(|_| typing)?;
        let input = self.blocks.input;
        self.pane.with_term(|term| {
            if term.on_alternate_screen() {
                return None;
            }
            let start = term.anchor_point(prompt, PointSpace::Viewport)?.row;
            let typed = input.and_then(|id| term.anchor_point(id, PointSpace::Viewport));
            Some(match typed {
                Some(at) if at.row > start && at.col.0 <= 1 => at.row - 1,
                Some(at) => at.row,
                None => start,
            })
        })
    }

    /// The bands to paint over the grid. Empty on the alternate screen.
    pub fn card_bands(&self, colours: &CardColours) -> Vec<RowBand> {
        // This frame's cursor row when the terminal knows it, else where the last paint put it.
        let cursor = self
            .pane
            .with_term(|term| term.cursor_viewport_row())
            .or_else(|| self.painter.cursor_cell().map(|c| c.row.0))
            .map(i64::from);
        let typing = matches!(
            self.blocks.machine,
            BlockState::Prompting { .. } | BlockState::Typing { .. }
        );
        self.pane.with_term(|term| {
            if term.on_alternate_screen() {
                return Vec::new();
            }
            let Ok((top, _)) = term.viewport() else {
                return Vec::new();
            };
            let top = i64::from(top);
            let rows = term.size().rows.0;
            let first_of = |anchor| {
                let at = term.anchor_point(anchor, PointSpace::Screen)?;
                Some(i64::from(at.row) - top)
            };
            let end = cursor.unwrap_or(i64::from(rows) - 1);
            // The rows of the command typed after the input mark, up to the output mark.
            let command_of = |input: Option<AnchorId>, output: Option<AnchorId>| {
                let input = first_of(input?)?;
                let output = output.and_then(|anchor| {
                    let at = term.anchor_point(anchor, PointSpace::Screen)?;
                    Some((i64::from(at.row) - top, at.col.0))
                });
                Some(command_rows(input, output, end))
            };
            let mut blocks: Vec<CardBlock> = self
                .blocks
                .store
                .iter()
                .filter_map(|block| {
                    let anchors = self.blocks.anchors.get(&block.id)?;
                    let prompt = anchors.prompt?;
                    let first = first_of(prompt)?;
                    let pick = match self.selected_block == Some(block.id) {
                        true => Pick::Selected,
                        false => Pick::Unselected,
                    };
                    Some(CardBlock {
                        first,
                        lead: lead_at(term, prompt, anchors.input),
                        stage: Stage::Ran(block.outcome()),
                        pick,
                        command: command_of(anchors.input, anchors.output),
                    })
                })
                .collect();
            let live = self.blocks.prompt.filter(|_| typing);
            if let Some((prompt, first)) = live.and_then(|p| Some((p, first_of(p)?))) {
                blocks.push(CardBlock {
                    first,
                    lead: lead_at(term, prompt, self.blocks.input),
                    stage: Stage::AtPrompt,
                    pick: Pick::Unselected,
                    command: command_of(self.blocks.input, None),
                });
            }
            blocks.sort_by_key(|b| b.first);
            bands_of(&blocks, end, rows, colours)
        })
    }
}
