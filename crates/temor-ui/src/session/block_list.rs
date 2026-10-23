//! The pane as a list of blocks (PLAN: block list): what the list shows, a finished block's rows
//! drawn whole as a still, and where the running block's live rows are in the pane's own picture.

use std::ops::Range;

use temor_core::session::list::{BlockFacts, Item, list_of};
use temor_render::{CellPos, PixelSize};
use temor_vt::{Col, Row};

use super::Session;
use super::model::INSET;
use crate::error::UiError;
use crate::paint::Inset;

/// The room between a card and the pane's sides, in logical pixels: a card's rows are drawn
/// this much narrower than the pane, with this much less inset, so its columns are the grid's.
pub const CARD_MARGIN: f32 = 8.0;

/// Whether panes are drawn as lists of blocks (`TEMOR_BLOCK_LIST=1`) instead of one grid with
/// cards over it.
pub fn block_list_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TEMOR_BLOCK_LIST").is_some_and(|v| v == "1"))
}

/// Where some of the viewport's rows are in the pane's picture, in its own pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRows {
    pub src_x: u32,
    pub src_y: u32,
    pub height: u32,
    pub width: u32,
}

impl Session {
    /// The blocks as the list shows them, oldest first. `None` while a full-screen program holds
    /// the screen: the pane shows that program's screen whole.
    pub fn block_list(&self) -> Option<Vec<Item>> {
        let mut read = self.survey()?;
        // A program running under the prompt draws below its cursor too (a status line under
        // an input box): its rows run to the last row anything is drawn on.
        let drawn = read
            .top
            .saturating_add(read.len.saturating_sub(1))
            .saturating_sub(u32::from(self.painter.lower()));
        read.cursor = Some(read.cursor.map_or(drawn, |cursor| cursor.max(drawn)));
        Some(list_of(&read, |id| BlockFacts {
            folder: self.header_facts(id).map(|kept| kept.folder.clone()),
            folded: self.block_folded(id),
        }))
    }

    /// One row's height in logical pixels (`None` before the first layout).
    pub fn row_height(&self) -> Option<f32> {
        let fit = self.fit?;
        Some(fit.cell.height.0 as f32 / self.scale.factor().max(f32::EPSILON))
    }

    /// Draws screen rows `rows` whole, as wide as a card, on `ground` where a cell has no
    /// colour of its own, handing the pixels to `show`.
    pub fn draw_rows(
        &mut self,
        rows: Range<u32>,
        ground: Option<temor_vt::Rgb>,
        show: impl FnOnce(&[u8], PixelSize),
    ) -> Result<(), UiError> {
        let (cols, cells) = self.pane.with_term(|term| {
            let cols = term.size().cols.0;
            (cols, term.styled_rows(rows))
        });
        let cells = cells?;
        let margin = self.scale.device(CARD_MARGIN).0;
        let width = self.pixels.width.0.saturating_sub(2 * margin);
        let inset = Inset {
            horizontal: INSET.horizontal - CARD_MARGIN,
            ..INSET
        };
        self.painter
            .still(cols, &cells, (width, self.scale, inset), ground, show)
    }

    /// Where screen rows `rows`, which the viewport shows, are in the pane's picture as the last
    /// paint drew it. `None` when none of them is in the viewport.
    pub fn live_rows(&self, rows: Range<u32>) -> Option<LiveRows> {
        let fit = self.placed()?.fit;
        let (top, len) = self.pane.with_term(|term| term.viewport().ok())?;
        let first = rows.start.max(top);
        let end = rows.end.min(top.saturating_add(len));
        if first >= end {
            return None;
        }
        let at = |row: u32| {
            let pos = CellPos {
                row: Row(u16::try_from(row - top).unwrap_or(u16::MAX)),
                col: Col(0),
            };
            fit.cell_origin(pos).1.0
        };
        let margin = self.scale.device(CARD_MARGIN).0;
        Some(LiveRows {
            src_x: margin,
            src_y: at(first),
            height: (end - first) * fit.cell.height.0,
            width: fit.pane.width.0.saturating_sub(2 * margin),
        })
    }
}
