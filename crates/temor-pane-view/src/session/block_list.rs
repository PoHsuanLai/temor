//! The pane as a list of blocks (PLAN: block list): what the list shows, a finished block's rows
//! drawn whole as a still, and where the running block's live rows are in the pane's own picture.

use std::ops::Range;

use temor_client::model::session::list::{BlockFacts, Item, KeptCard, list_of};
use temor_client::model::vocab::ids::BlockId;
use temor_panes::pane::text::CellPos as ScreenCell;
use temor_panes::render::{CellPos, HitRegion, PixelSize};
use temor_panes::vt::{Col, Row, SurfacePoint};

use super::Session;
use super::list_hit::{Landing, ListMap, list_hit};
use super::model::INSET;
use super::pointer::screen_cell;
use crate::error::ViewError;
use crate::paint::Inset;

/// The room between a card and the pane's sides, in logical pixels: a card's rows are drawn
/// this much narrower than the pane, with this much less inset, so its columns are the grid's.
pub const CARD_MARGIN: f32 = 8.0;

/// The screen cell under a point in the pane, as the pane draws it: one grid or a list of blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointHit {
    pub cell: ScreenCell,
    /// Whether the point is over the cells' columns, not the room beside them.
    pub region: HitRegion,
    /// The block the list put there, when the pane is a list: there the block a point is over
    /// is the card it is on, which may have no rows to find it by.
    pub listed: Option<Option<BlockId>>,
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
        let own = list_of(&read, |id| BlockFacts {
            folded: self.block_folded(id),
            bookmarked: self.block_bookmarked(id),
        });
        let kept = self.kept.iter().cloned().map(Item::Kept);
        Some(kept.chain(own).collect())
    }

    /// Shows `kept`, the commands the session ran before it was reopened, above the pane's own.
    pub fn set_kept(&mut self, kept: Vec<KeptCard>) {
        self.kept = kept;
    }

    /// Puts kept command `n` in the prompt, as Put in Prompt does a block's.
    pub fn put_kept(&mut self, n: u32) {
        let Some(card) = self.kept.iter().find(|card| card.n == n) else {
            return;
        };
        let command = temor_client::model::vocab::text::CommandText(card.command.clone());
        if !self.put_in_prompt(command) {
            self.notice = Some("This command cannot be typed again.".to_owned());
        }
    }

    /// The list as its view laid it out now, or `None` while the pane is one grid.
    pub fn set_list_map(&mut self, map: Option<ListMap>) {
        self.list_map = map;
    }

    /// The ground the live screen is drawn on: the running card's while the pane is a list, the
    /// terminal's own (`None`) while it is one grid. Whether it changed: the pane is drawn again.
    pub fn set_live_ground(&mut self, ground: Option<temor_panes::vt::Rgb>) -> bool {
        let changed = self.live_ground != ground;
        self.live_ground = ground;
        changed
    }

    /// How far the list is scrolled now, in logical pixels.
    pub fn set_list_offset(&mut self, offset: f32) {
        if let Some(map) = self.list_map.as_mut() {
            map.offset = offset;
        }
    }

    /// The screen cell under `at` (device pixels in the pane). `None` before the first layout.
    pub(crate) fn point_hit(&self, at: SurfacePoint) -> Option<PointHit> {
        let placed = self.placed()?;
        let Some(map) = &self.list_map else {
            let hit = placed.cell_at(at);
            let top = self.viewport_top().ok()?;
            return Some(PointHit {
                cell: screen_cell(top, hit.pos.row.0, hit.pos.col.0),
                region: hit.region,
                listed: None,
            });
        };
        let factor = self.scale.factor().max(f32::EPSILON);
        let row = placed.fit.cell.height.0 as f32 / factor;
        let found = list_hit(map, at.y as f32 / factor, row)?;
        // The list's rows keep the grid's columns: only the point's height is the list's.
        let hit = placed.fit.cell_at(at);
        let left = i64::from(placed.fit.padding.left.0);
        let width = i64::from(placed.fit.grid.cols.0) * i64::from(placed.fit.cell.width.0);
        let over = (left..left + width).contains(&i64::from(at.x));
        let last = placed.fit.grid.cols.0.saturating_sub(1);
        let (row, col, region) = match found.landing {
            Some(Landing::On(row)) if over => (row, hit.pos.col.0, HitRegion::Grid),
            Some(Landing::On(row)) => (row, hit.pos.col.0, HitRegion::Padding),
            Some(Landing::Start(row)) => (row, 0, HitRegion::Padding),
            Some(Landing::End(row)) => (row, last, HitRegion::Padding),
            // No card shows a row: the point is on its block's card, whose first row is its own.
            None => (
                self.frame()?.block_first_row(found.block?)?,
                0,
                HitRegion::Padding,
            ),
        };
        Some(PointHit {
            cell: ScreenCell::new(row, col),
            region,
            listed: Some(found.block),
        })
    }

    /// The block a hit is on.
    pub(crate) fn hit_block(&self, hit: &PointHit) -> Option<BlockId> {
        match hit.listed {
            Some(block) => block,
            None => self.frame()?.block_at_screen_row(hit.cell.row),
        }
    }

    /// The point a program that has the mouse is told of for `at`: the cell's place in the
    /// viewport, where the program drew it.
    pub(crate) fn drawn_point(&self, at: SurfacePoint) -> Option<SurfacePoint> {
        let placed = self.placed()?;
        if self.list_map.is_none() {
            return Some(placed.drawn_point(at));
        }
        let hit = self.point_hit(at)?;
        let top = self.viewport_top().ok()?;
        let row = hit.cell.row.saturating_sub(top);
        let pos = CellPos {
            row: Row(u16::try_from(row).unwrap_or(u16::MAX)),
            col: Col(hit.cell.col),
        };
        let (x, y) = placed.fit.cell_origin(pos);
        let half = placed.fit.cell.height.0 / 2;
        Some(SurfacePoint {
            x: i32::try_from(x.0).unwrap_or(i32::MAX).max(at.x),
            y: i32::try_from(y.0 + half).unwrap_or(i32::MAX),
        })
    }

    /// One row's height in logical pixels (`None` before the first layout).
    pub fn row_height(&self) -> Option<f32> {
        let fit = self.fit?;
        Some(fit.cell.height.0 as f32 / self.scale.factor().max(f32::EPSILON))
    }

    /// What of screen rows `rows` the selection covers, as its ends (or `None`): a drawing of
    /// them is drawn again when it changes.
    pub fn selection_over(&self, rows: Range<u32>) -> Option<(ScreenCell, ScreenCell)> {
        let selection = self.selection?;
        let (start, end) = selection.ends();
        (start.row < rows.end && end.row >= rows.start).then_some((start, end))
    }

    /// Draws screen rows `rows` whole, as wide as a card, on `ground` where a cell has no
    /// colour of its own, with the selection over them, handing the pixels to `show`.
    pub fn draw_rows(
        &mut self,
        rows: Range<u32>,
        ground: Option<temor_panes::vt::Rgb>,
        show: impl FnOnce(&[u8], PixelSize),
    ) -> Result<(), ViewError> {
        let (cols, cells) = self.pane.with_term(|term| {
            let cols = term.size().cols.0;
            (cols, term.styled_rows(rows.clone()))
        });
        let mut cells = cells?;
        if let Some(selection) = self.selection {
            for (row, line) in (rows.start..).zip(cells.iter_mut()) {
                for cell in line {
                    if selection.contains(ScreenCell::new(row, cell.col.0)) {
                        cell.selected = temor_panes::vt::Selected::Yes;
                    }
                }
            }
        }
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
