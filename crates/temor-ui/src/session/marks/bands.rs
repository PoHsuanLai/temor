//! The command cards as row bands for the renderer: a rule above every block, a stripe and a
//! faint fill on a failed one, an accent fill on the selected one, and a faint fill on the line
//! being typed. `bands_of` is pure; `Session::card_bands` gathers its input from the terminal.

use temor_core::block::{BlockState, Outcome};
use temor_render::{RowBand, RowSpan};
use temor_vt::{PointSpace, Row};

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

/// One block: its first row in the viewport (negative above it) and how it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CardBlock {
    pub first: i64,
    pub stage: Stage,
    pub pick: Pick,
}

/// The bands for `blocks` (oldest first) on a viewport of `rows`, the last block running to
/// viewport row `end`. Each block runs to the row before the next one's first row.
pub(super) fn bands_of(
    blocks: &[CardBlock],
    end: i64,
    rows: u16,
    colours: &CardColours,
) -> Vec<RowBand> {
    let mut out = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        let last = blocks
            .get(i + 1)
            .map_or(end, |next| next.first - 1)
            .max(block.first);
        let top = block.first.max(0);
        let bottom = last.min(i64::from(rows) - 1);
        if top > bottom {
            continue;
        }
        let fill = match (block.pick, block.stage) {
            (Pick::Selected, _) => Some(colours.selected_fill),
            (Pick::Unselected, Stage::Ran(Outcome::Failed(_))) => Some(colours.failed_fill),
            (Pick::Unselected, Stage::AtPrompt) => Some(colours.input),
            (Pick::Unselected, Stage::Ran(_)) => None,
        };
        let stripe = match block.stage {
            Stage::Ran(Outcome::Failed(_)) => Some(colours.failed_stripe),
            _ => None,
        };
        // A block whose first row is row 0 (or above the view) shows no rule.
        let rule = (block.first >= 1).then_some(colours.rule);
        if fill.is_none() && stripe.is_none() && rule.is_none() {
            continue;
        }
        out.push(RowBand {
            rows: RowSpan {
                first: Row(top as u16),
                last: Row(bottom as u16),
            },
            fill,
            stripe,
            rule,
        });
    }
    out
}

impl Session {
    /// The bands to paint over the grid. Empty on the alternate screen.
    pub fn card_bands(&self, colours: &CardColours) -> Vec<RowBand> {
        let cursor = self.painter.cursor_cell().map(|c| i64::from(c.row.0));
        let typing = matches!(
            self.blocks.machine,
            BlockState::Prompting { .. } | BlockState::Typing { .. }
        );
        self.pane.with_term(|term| {
            if term.on_alternate_screen() {
                return Vec::new();
            }
            let Ok(bar) = term.scrollbar() else {
                return Vec::new();
            };
            let top = i64::try_from(bar.offset.0).unwrap_or(i64::MAX);
            let rows = term.size().rows.0;
            let first_of = |anchor| {
                let at = term.anchor_point(anchor, PointSpace::Screen)?;
                Some(i64::from(at.row) - top)
            };
            let mut blocks: Vec<CardBlock> = self
                .blocks
                .store
                .iter()
                .filter_map(|block| {
                    let first = first_of(self.blocks.anchors.get(&block.id)?.prompt?)?;
                    let pick = match self.selected_block == Some(block.id) {
                        true => Pick::Selected,
                        false => Pick::Unselected,
                    };
                    Some(CardBlock {
                        first,
                        stage: Stage::Ran(block.outcome()),
                        pick,
                    })
                })
                .collect();
            if let (true, Some(first)) = (typing, self.blocks.prompt.and_then(first_of)) {
                blocks.push(CardBlock {
                    first,
                    stage: Stage::AtPrompt,
                    pick: Pick::Unselected,
                });
            }
            blocks.sort_by_key(|b| b.first);
            let end = cursor.unwrap_or(i64::from(rows) - 1);
            bands_of(&blocks, end, rows, colours)
        })
    }
}
