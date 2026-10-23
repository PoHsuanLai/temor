//! Where the grid's rows are shown (`temor_render::RowLayout`): folded blocks' output rows taken
//! out, and the rows pinned to the top at the start of the scrollback. Every place that maps a
//! point to a cell reads the rows where they are shown.

use temor_render::{
    CellHit, CellPos, CellSide, Fold, GridFit, GridHit, HitRegion, LayoutParams, Pin, RowLayout,
    Screen,
};
use temor_vt::{Col, Row, SurfacePoint};

use super::Session;

/// The most rows a lens's picture takes in its block's card.
pub const LENS_MOST_ROWS: u32 = 24;

/// Whether the rows moved since the last layout: the window draws the slices again then, so they
/// never drift from the picture of the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relaid {
    Same,
    Moved,
}

/// The grid as the last paint placed it, and where its rows are shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The grid as drawn: fitted and lowered over the blank rows under the screen.
    pub fit: GridFit,
    pub layout: RowLayout,
}

impl Placed {
    /// The cell under a pane point. A point in the room under a block is taken as the end of
    /// the block's last row (a drag into it selects to there), outside the grid.
    pub fn cell_at(&self, at: SurfacePoint) -> CellHit {
        match self.layout.to_grid(at) {
            GridHit::Gap { after } => CellHit {
                pos: CellPos {
                    row: after,
                    col: Col(self.fit.grid.cols.0.saturating_sub(1)),
                },
                side: CellSide::Right,
                region: HitRegion::Padding,
            },
            GridHit::Grid(drawn) => self.fit.cell_at(drawn),
            _ => self.fit.cell_at(at),
        }
    }

    /// The point of the drawn grid a pane point is over, for reports to a program that has the
    /// mouse: in the room under a block, the last cell of the block's last row.
    pub fn drawn_point(&self, at: SurfacePoint) -> SurfacePoint {
        match self.layout.to_grid(at) {
            GridHit::Grid(drawn) => drawn,
            GridHit::Gap { after } => {
                let (x, y) = self.fit.cell_origin(CellPos {
                    row: after,
                    col: Col(self.fit.grid.cols.0.saturating_sub(1)),
                });
                SurfacePoint {
                    x: i32::try_from(x.0).unwrap_or(i32::MAX),
                    y: i32::try_from(y.0).unwrap_or(i32::MAX),
                }
            }
            _ => at,
        }
    }
}

impl Session {
    /// Where the rows are shown, as of the last paint (`None` before the first layout).
    pub fn row_layout(&self) -> Option<&RowLayout> {
        self.layout.as_ref()
    }

    /// Lays the rows out around the room under the blocks with pictures, as they are on the
    /// screen now. Whether the rows moved.
    pub(super) fn relayout(&mut self) -> Relaid {
        let Some(fit) = self.fit else {
            return Relaid::Same;
        };
        let lower = self.painter.lower();
        let placed = fit.lowered(lower);
        let (alternate, viewport, total) = self.pane.with_term(|term| {
            (
                term.on_alternate_screen(),
                term.viewport().ok(),
                term.screen_rows().ok(),
            )
        });
        let frame = self.frame().filter(|_| !alternate);
        // Folded blocks' output rows, cut to the rows on screen.
        let rows = i64::from(fit.grid.rows.0);
        let folded: Vec<Fold> = match alternate {
            true => Vec::new(),
            false => self
                .ann
                .folded()
                .filter(|&id| self.block_finished(id))
                .filter_map(|id| {
                    let (first, last) = frame.as_ref()?.output_rows(id)?;
                    let (first, last) = (first.max(0), last.min(rows - 1));
                    if first > last {
                        return None;
                    }
                    Some(Fold {
                        first: Row(u16::try_from(first).ok()?),
                        last: Row(u16::try_from(last).ok()?),
                    })
                })
                .collect(),
        };
        // At the start of the scrollback, scrolled back: the first rows must stay reachable.
        let pin = match (viewport, total) {
            (Some((0, len)), Some(total)) if len < total => Pin::Top,
            _ => Pin::Bottom,
        };
        let layout = RowLayout::new(LayoutParams {
            fit: placed,
            lower,
            inserts: &[],
            folded: &folded,
            pin,
            screen: match alternate {
                true => Screen::Alternate,
                false => Screen::Primary,
            },
        });
        match self.layout.as_ref() == Some(&layout) {
            true => Relaid::Same,
            false => {
                self.layout = Some(layout);
                Relaid::Moved
            }
        }
    }

    /// The grid as the last paint placed it and where its rows are shown (`None` before the
    /// first layout). Before the first paint the rows are where they are drawn.
    pub(super) fn placed(&self) -> Option<Placed> {
        let fit = self.fit?.lowered(self.painter.lower());
        let layout = self
            .layout
            .clone()
            .filter(|layout| layout.fit() == fit)
            .unwrap_or_else(|| RowLayout::identity(fit));
        Some(Placed { fit, layout })
    }
}
