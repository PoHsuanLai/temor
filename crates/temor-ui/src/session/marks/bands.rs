//! The command cards as row bands for the renderer: a rule above every block (the line being
//! typed is set off from history by the rule above its block, with no fill of its own), a
//! stripe and a faint fill on a failed one, an accent fill on the selected one, and the command
//! line in bold. temor's prompt leaves a blank line above its header; the card's edge runs
//! across the middle of that line, so every card has half a line of room above its header and
//! half a line below its last row, as Warp's blocks do. `bands_of` is pure;
//! `Session::card_bands` gathers its input from the terminal.

use temor_core::block::{BlockState, Outcome};
use temor_core::session::frame::{CardBlock, Lead, Mark, Pick, Stage};
use temor_pane::Term;
use temor_pane::text::RowText;
use temor_render::{Cut, RowBand, RowSpan, RowWeight};
use temor_vt::{AnchorId, Col, PointSpace, Row};

use super::super::Session;
use crate::view::card_colours::CardColours;

/// Whether a prompt's header is a line of its own (temor's prompt) or the line typed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderLine {
    Own,
    Shared,
}

/// The header line of the prompt being typed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptHeader {
    /// Its viewport row.
    pub row: u32,
    pub line: HeaderLine,
    /// What the shell wrote on it, without trailing blanks.
    pub text: String,
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
        // A bookmark's stripe wins over a failure's: the person put it there.
        let stripe = match (block.mark, block.stage) {
            (Mark::Bookmarked, _) => Some(colours.bookmark_stripe),
            (Mark::Plain, Stage::Ran(Outcome::Failed(_))) => Some(colours.failed_stripe),
            (Mark::Plain, _) => None,
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
        // The command line draws bold, clipped to the view, from where the command starts: a
        // one-line prompt before it on its row stays as the shell drew it.
        if let Some((first, to, col)) = block.command {
            let (from, to) = (first.max(0), to.min(i64::from(rows) - 1));
            let weight = match (from == first, col) {
                (true, 1..) => RowWeight::BoldFrom(Col(col)),
                _ => RowWeight::Bold,
            };
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
                    weight,
                });
            }
        }
    }
    out
}

impl Session {
    /// The header line of the prompt being typed at: with temor's prompt, the line above the
    /// empty typing line; with a one-line prompt, the prompt's own line. None while a command
    /// runs, on the alternate screen, or when it is out of view.
    pub fn live_prompt_header(&self) -> Option<PromptHeader> {
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
            // The header in one point space: its row, and whether it is a line of its own.
            let header_in = |space| {
                let start = term.anchor_point(prompt, space)?.row;
                let typed = input.and_then(|id| term.anchor_point(id, space));
                Some(match typed {
                    Some(at) if at.row > start && at.col.0 <= 1 => (at.row - 1, HeaderLine::Own),
                    Some(at) => (at.row, HeaderLine::Shared),
                    None => (start, HeaderLine::Shared),
                })
            };
            let (row, line) = header_in(PointSpace::Viewport)?;
            let text = header_in(PointSpace::Active)
                .and_then(|(at, _)| term.row_text(at).ok())
                .map(|text| text.text().trim_end().to_owned())
                .unwrap_or_default();
            Some(PromptHeader { row, line, text })
        })
    }

    /// The bands to paint over the grid. Empty on the alternate screen.
    pub fn card_bands(&self, colours: &CardColours) -> Vec<RowBand> {
        let Some(frame) = self.frame() else {
            return Vec::new();
        };
        let rows = self.pane.with_term(|term| term.size().rows.0);
        // Where a running command's rows end: the cursor's row, else the viewport's last.
        let end = frame.cursor_row().unwrap_or(i64::from(rows) - 1);
        bands_of(&frame.cards(end), end, rows, colours)
    }
}
