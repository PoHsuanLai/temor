//! Where the grid's rows are shown (`temor_panes::render::RowLayout`): folded blocks' output rows taken
//! out, and the rows pinned to the top at the start of the scrollback. Every place that maps a
//! point to a cell reads the rows where they are shown.

use temor_panes::render::{CellHit, Fold, GridFit, LayoutParams, Pin, RowLayout, Screen};
use temor_panes::vt::{Row, SurfacePoint};

use super::Session;

/// The most rows a lens's picture takes in its block's card.
pub const LENS_MOST_ROWS: u32 = 24;

/// The grid as the last paint placed it, and where its rows are shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The grid as drawn: fitted and lowered over the blank rows under the screen.
    pub fit: GridFit,
    pub layout: RowLayout,
}

impl Placed {
    /// The cell under a pane point.
    pub fn cell_at(&self, at: SurfacePoint) -> CellHit {
        self.fit.cell_at(self.drawn_point(at))
    }

    /// The point of the drawn grid a pane point is over, for reports to a program that has the
    /// mouse.
    pub fn drawn_point(&self, at: SurfacePoint) -> SurfacePoint {
        self.layout.to_grid(at)
    }
}

impl Session {
    /// Where the rows are shown, as of the last paint (`None` before the first layout).
    pub fn row_layout(&self) -> Option<&RowLayout> {
        self.layout.as_ref()
    }

    /// Lays the rows out without the folded blocks' output, as they are on the screen now. True
    /// when the rows moved since the last layout: the window draws the slices again then, so they
    /// never drift from the picture of the grid.
    pub(super) fn relayout(&mut self) -> bool {
        let Some(fit) = self.fit else {
            return false;
        };
        let placed = fit.lowered(self.painter.lower());
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
            folded: &folded,
            pin,
            screen: match alternate {
                true => Screen::Alternate,
                false => Screen::Primary,
            },
        });
        let moved = self.layout.as_ref() != Some(&layout);
        self.layout = Some(layout);
        moved
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
