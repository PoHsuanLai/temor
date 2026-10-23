//! Where a block's anchors are on the screen: rows, the scrollback dropping them, the text
//! between two of them, and the sweep that drops blocks the scrollback has taken.

use temor_core::vocab::ids::BlockId;
use temor_vt::{AnchorId, PointSpace};

use super::super::Session;

impl Session {
    /// The row of an anchor in the screen (0 is the oldest kept row); `None` once it is gone.
    pub(super) fn screen_row(&self, id: AnchorId) -> Option<u32> {
        self.pane
            .with_term(|term| term.anchor_point(id, PointSpace::Screen))
            .map(|at| at.row)
    }

    /// Hands an anchor back to the terminal.
    pub(super) fn drop_anchor(&self, id: AnchorId) {
        self.pane.with_term(|term| term.release_anchor(id));
    }

    /// The text from one anchor up to (not including) another, trimmed of the line ends around
    /// it. The end cell is excluded: an anchor at a line's start marks the end of the line above.
    pub(super) fn text_between(&self, from: AnchorId, to: AnchorId) -> Option<String> {
        self.pane.with_term(|term| {
            let start = term.anchor_point(from, PointSpace::Screen)?;
            let end = term.anchor_point(to, PointSpace::Screen)?;
            let last_col = term.size().cols.0.saturating_sub(1);
            let stop = match (end.col.0, end.row.checked_sub(1)) {
                (0, Some(row)) if end.row > start.row => {
                    temor_vt::GridPoint::new(PointSpace::Screen, last_col, row)
                }
                (col, _) if col > 0 && end.row >= start.row => {
                    temor_vt::GridPoint::new(PointSpace::Screen, col - 1, end.row)
                }
                _ => return Some(String::new()),
            };
            let text = term.text(start, stop).ok()?;
            Some(text.trim().to_owned())
        })
    }

    /// The blocks whose prompt and output the scrollback no longer holds.
    pub(super) fn swept(&self) -> Vec<BlockId> {
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

    /// The newest block whose prompt..end span holds screen row `row`.
    pub(super) fn block_at_row(&self, row: u32) -> Option<BlockId> {
        self.blocks.store().iter().rev().find_map(|block| {
            let (first, last) = self.block_rows(block.id)?;
            (first..=last).contains(&row).then_some(block.id)
        })
    }

    /// The first and last screen rows of a block (a running block runs to the bottom).
    pub(super) fn block_rows(&self, id: BlockId) -> Option<(u32, u32)> {
        let anchors = self.blocks.anchors_of(id)?;
        let first = self.screen_row(anchors.prompt.or(anchors.output)?)?;
        let last = match anchors.end {
            Some(end) => self.screen_row(end)?,
            None => u32::MAX,
        };
        Some((first, last))
    }
}
