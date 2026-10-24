//! Rows left out of a grid's picture: the grid is drawn as one picture of whole rows, and folded
//! rows (a folded command's output) are left out by showing that picture in slices.
//! [`RowLayout`] is the geometry: where each slice of the drawn grid goes in the pane, and which
//! drawn point a pane point is over. It never redraws anything; the grid is drawn as it always
//! is, and only shown cut apart.
//!
//! The rows keep their order and their size. A fold moves the rows above it down
//! ([`Pin::Bottom`], the live screen, whose last row stays on the pane's bottom) or the rows
//! below it up ([`Pin::Top`], for the start of the scrollback, whose first rows stay where they
//! are). What is moved out of the pane is cut.

use std::ops::Range;

use temor_vt::{Px, Row, SurfacePoint};

use super::layout_types::{LayoutParams, Pin, Screen, Slice};
use super::model::GridFit;
use crate::track::band_of;

/// Rows `first..=last`, shown `shift` device pixels from where they are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Run {
    first: u16,
    last: u16,
    /// The drawn device rows of the run: its rows' bands (`band_of`), so the first and last
    /// runs take in the padding.
    src: (i64, i64),
    shift: i64,
    /// Left out of the picture: it takes no room, and its rows are shown where it began.
    folded: bool,
}

/// Where a grid's rows are shown when some are left out. With none left out, the picture is shown
/// whole where it was drawn and every point is the point it was. `Send` and `Sync`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowLayout {
    fit: GridFit,
    runs: Vec<Run>,
    slices: Vec<Slice>,
    visible: Range<u16>,
}

fn px(v: i64) -> Px {
    Px(u32::try_from(v.max(0)).unwrap_or(u32::MAX))
}

fn clamp_i32(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

impl RowLayout {
    /// The grid shown as it is drawn: one slice.
    pub fn identity(fit: GridFit) -> Self {
        Self::new(LayoutParams {
            fit,
            folded: &[],
            pin: Pin::Bottom,
            screen: Screen::Primary,
        })
    }

    /// Lays the rows out without `params.folded`: a row's shown top is its drawn top, less the
    /// folded rows above it, plus what the pin pushes down (all the folded rows for
    /// [`Pin::Bottom`], none for [`Pin::Top`]).
    pub fn new(params: LayoutParams<'_>) -> Self {
        let fit = params.fit;
        let rows = fit.grid.rows.0.max(1);
        let ch = i64::from(fit.cell.height.0.max(1));
        let primary = params.screen == Screen::Primary;
        let folded_row = |row: u16| {
            primary
                && params
                    .folded
                    .iter()
                    .any(|f| f.first.0 <= row && row <= f.last.0)
        };
        let folded_total = (0..rows).filter(|&row| folded_row(row)).count() as i64 * ch;
        let push = match params.pin {
            Pin::Bottom => folded_total,
            Pin::Top => 0,
        };
        let (top, pane) = (
            clamp_i32(i64::from(fit.padding.top.0)),
            clamp_i32(i64::from(fit.pane.height.0)),
        );
        let band = |row: u16| {
            let (start, end) = band_of(Row(row), rows, top, clamp_i32(ch), pane);
            (i64::from(start), i64::from(end))
        };
        let run = |first: u16, last: u16, shift: i64, folded: bool| Run {
            first,
            last,
            src: (band(first).0, band(last).1),
            shift,
            folded,
        };
        let mut runs = Vec::new();
        let (mut first, mut above) = (0u16, 0i64);
        for row in 0..rows {
            if row + 1 == rows || folded_row(row + 1) != folded_row(row) {
                let folded = folded_row(row);
                runs.push(run(first, row, above + push, folded));
                if folded {
                    above -= i64::from(row - first + 1) * ch;
                }
                first = row + 1;
            }
        }
        let pane = i64::from(pane);
        let cut = |start: i64, end: i64| (start.max(0), end.min(pane));
        let mut slices = Vec::with_capacity(runs.len());
        for r in runs.iter().filter(|r| !r.folded) {
            let (start, end) = cut(r.src.0 + r.shift, r.src.1 + r.shift);
            if start < end {
                slices.push(Slice {
                    src_y: px(start - r.shift),
                    height: px(end - start),
                    dst_y: px(start),
                });
            }
        }
        let mut layout = RowLayout {
            fit,
            runs,
            slices,
            visible: 0..0,
        };
        let whole: Vec<u16> = (0..rows)
            .filter(|&row| {
                if layout.run_of(row).folded {
                    return false;
                }
                let t = layout.top_of(row);
                t >= 0 && t + ch <= pane
            })
            .collect();
        layout.visible = match (whole.first(), whole.last()) {
            (Some(&a), Some(&b)) => a..b + 1,
            _ => 0..0,
        };
        layout
    }

    /// The grid this layout shows.
    pub fn fit(&self) -> GridFit {
        self.fit
    }

    /// The bands of the drawn grid to show, top to bottom.
    pub fn slices(&self) -> &[Slice] {
        &self.slices
    }

    /// The rows shown whole in the pane.
    pub fn visible_rows(&self) -> Range<u16> {
        self.visible.clone()
    }

    fn run_of(&self, row: u16) -> &Run {
        let found = self.runs.iter().find(|r| row <= r.last);
        // `new` always makes one run: the grid has at least one row.
        found.unwrap_or_else(|| &self.runs[self.runs.len() - 1])
    }

    /// How far row `row` is shown from where it is drawn, in device pixels: its run's shift, and
    /// for a folded row, back to where the fold began.
    fn shift_of(&self, row: u16) -> i64 {
        let run = self.run_of(row);
        match run.folded {
            true => {
                run.shift - i64::from(row - run.first) * i64::from(self.fit.cell.height.0.max(1))
            }
            false => run.shift,
        }
    }

    fn top_of(&self, row: u16) -> i64 {
        let drawn = i64::from(self.fit.padding.top.0)
            + i64::from(row) * i64::from(self.fit.cell.height.0.max(1));
        drawn + self.shift_of(row)
    }

    /// Whether any row is left out.
    pub fn has_folds(&self) -> bool {
        self.runs.iter().any(|r| r.folded)
    }

    /// Where row `row`'s top is shown, in device pixels from the pane's top (negative above it).
    /// A row past the grid's last is placed as if the grid went on.
    pub fn row_top(&self, row: Row) -> i32 {
        clamp_i32(self.top_of(row.0))
    }

    /// How far row `row` is shown from where it is drawn, in device pixels (up is negative).
    pub fn row_shift(&self, row: Row) -> i32 {
        clamp_i32(self.shift_of(row.0))
    }

    /// The point of the drawn grid the pane point `at` is over (map it to a cell with
    /// [`GridFit::cell_at`]). A point above or below the pane is over what the edge is over,
    /// carried on.
    pub fn to_grid(&self, at: SurfacePoint) -> SurfacePoint {
        let y = i64::from(at.y);
        let drawn = |r: &Run| SurfacePoint {
            x: at.x,
            y: clamp_i32(y - r.shift),
        };
        for r in &self.runs {
            // A folded run has no extent: it ends where it begins.
            let end = match r.folded {
                true => r.src.0 + r.shift,
                false => r.src.1 + r.shift,
            };
            if y < end {
                return drawn(r);
            }
        }
        drawn(&self.runs[self.runs.len() - 1])
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
