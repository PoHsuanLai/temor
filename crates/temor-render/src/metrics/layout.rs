//! Room between a grid's rows: the grid is drawn as one picture of whole rows, and a caller that
//! wants space under a row (a picture under a command's output) shows that picture in slices with
//! gaps between them. [`RowLayout`] is the geometry: where each slice of the drawn grid goes in
//! the pane, where each gap is, and which drawn point a pane point is over. It never redraws
//! anything; the grid is drawn as it always is, and only shown cut apart.
//!
//! The rows keep their order and their size. Room under a row pushes the rows above it up
//! ([`Pin::Bottom`], the live screen, whose last row stays on the pane's bottom) or the rows
//! below it down ([`Pin::Top`], for the start of the scrollback, whose first rows would otherwise
//! be out of reach). Either way the blank rows the grid was lowered over
//! ([`GridFit::lowered`]) are taken first. What is pushed out of the pane is cut.

use std::collections::BTreeMap;
use std::ops::Range;

use temor_vt::{Px, Row, SurfacePoint};

use super::model::GridFit;
use crate::track::band_of;

/// Room of `height` device pixels under grid row `after`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Insert {
    pub after: Row,
    pub height: Px,
}

/// Which end of the pane the rows hold on to when room is made between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Pin {
    /// The last row stays where it is: the rows above the room move up, and those pushed above
    /// the pane are cut.
    #[default]
    Bottom,
    /// The first rows stay reachable: the rows move up only into the blank rows the grid was
    /// lowered over, and the rest of the room pushes the rows below it down.
    Top,
}

/// Which of the terminal's screens the grid shows. A full-screen program's screen has no room
/// made in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Screen {
    #[default]
    Primary,
    Alternate,
}

/// A band of the drawn grid shown in the pane: drawn device rows `src_y..src_y + height`, shown
/// from `dst_y` down. Always inside the pane and the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Slice {
    pub src_y: Px,
    pub height: Px,
    pub dst_y: Px,
}

/// The room under row `after` as it is shown: device rows `dst_y..dst_y + height` of the pane,
/// cut to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Gap {
    pub after: Row,
    pub dst_y: Px,
    pub height: Px,
}

/// What a pane point is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GridHit {
    /// The drawn grid, at this point of the picture (map it to a cell with
    /// [`GridFit::cell_at`]).
    Grid(SurfacePoint),
    /// The room under row `after`.
    Gap { after: Row },
}

/// What a [`RowLayout`] is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutParams<'a> {
    /// The grid as it is drawn: fitted, and lowered by `lower`.
    pub fit: GridFit,
    /// How many blank rows the grid was lowered over.
    pub lower: u16,
    /// The room to make; inserts under rows the grid does not have are left out.
    pub inserts: &'a [Insert],
    pub pin: Pin,
    pub screen: Screen,
}

/// Rows `first..=last`, shown `shift` device pixels from where they are drawn, with `gap`
/// pixels of room under the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Run {
    first: u16,
    last: u16,
    /// The drawn device rows of the run: its rows' bands (`band_of`), so the first and last
    /// runs take in the padding.
    src: (i64, i64),
    shift: i64,
    gap: i64,
}

/// Where a grid's rows are shown when room is made between them. With no room, the picture is
/// shown whole where it was drawn and every point is the point it was. `Send` and `Sync`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowLayout {
    fit: GridFit,
    runs: Vec<Run>,
    slices: Vec<Slice>,
    gaps: Vec<Gap>,
    visible: Range<u16>,
}

fn px(v: i64) -> Px {
    Px(u32::try_from(v.max(0)).unwrap_or(u32::MAX))
}

fn clamp_i32(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

impl RowLayout {
    /// The grid shown as it is drawn: one slice, no room.
    pub fn identity(fit: GridFit) -> Self {
        Self::new(LayoutParams {
            fit,
            lower: 0,
            inserts: &[],
            pin: Pin::Bottom,
            screen: Screen::Primary,
        })
    }

    /// Lays the rows out around `params.inserts`: a row's shown top is its drawn top, plus the
    /// room above it, less what the pin pulls up (all the room for [`Pin::Bottom`], at most the
    /// blank rows the grid was lowered over for [`Pin::Top`]).
    pub fn new(params: LayoutParams<'_>) -> Self {
        let fit = params.fit;
        let rows = fit.grid.rows.0.max(1);
        let ch = i64::from(fit.cell.height.0.max(1));
        let mut room: BTreeMap<u16, i64> = BTreeMap::new();
        if params.screen == Screen::Primary {
            for insert in params.inserts {
                if insert.after.0 < rows && insert.height.0 > 0 {
                    *room.entry(insert.after.0).or_default() += i64::from(insert.height.0);
                }
            }
        }
        let total: i64 = room.values().sum();
        let pull = match params.pin {
            Pin::Bottom => total,
            Pin::Top => total.min(i64::from(params.lower) * ch),
        };
        let (top, pane) = (
            clamp_i32(i64::from(fit.padding.top.0)),
            clamp_i32(i64::from(fit.pane.height.0)),
        );
        let band = |row: u16| {
            let (start, end) = band_of(Row(row), rows, top, clamp_i32(ch), pane);
            (i64::from(start), i64::from(end))
        };
        let run = |first: u16, last: u16, shift: i64, gap: i64| Run {
            first,
            last,
            src: (band(first).0, band(last).1),
            shift,
            gap,
        };
        let mut runs = Vec::with_capacity(room.len() + 1);
        let (mut first, mut above) = (0u16, 0i64);
        for (&after, &height) in &room {
            runs.push(run(first, after, above - pull, height));
            above += height;
            first = after + 1;
        }
        if first < rows {
            runs.push(run(first, rows - 1, above - pull, 0));
        }
        let pane = i64::from(pane);
        let cut = |start: i64, end: i64| (start.max(0), end.min(pane));
        let mut slices = Vec::with_capacity(runs.len());
        let mut gaps = Vec::with_capacity(room.len());
        for r in &runs {
            let (start, end) = cut(r.src.0 + r.shift, r.src.1 + r.shift);
            if start < end {
                slices.push(Slice {
                    src_y: px(start - r.shift),
                    height: px(end - start),
                    dst_y: px(start),
                });
            }
            let (start, end) = cut(r.src.1 + r.shift, r.src.1 + r.shift + r.gap);
            if r.gap > 0 && start < end {
                gaps.push(Gap {
                    after: Row(r.last),
                    dst_y: px(start),
                    height: px(end - start),
                });
            }
        }
        let mut layout = RowLayout {
            fit,
            runs,
            slices,
            gaps,
            visible: 0..0,
        };
        let whole: Vec<u16> = (0..rows)
            .filter(|&row| {
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

    /// The room between them, top to bottom; empty when none was made.
    pub fn gaps(&self) -> &[Gap] {
        &self.gaps
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

    fn top_of(&self, row: u16) -> i64 {
        let drawn = i64::from(self.fit.padding.top.0)
            + i64::from(row) * i64::from(self.fit.cell.height.0.max(1));
        drawn + self.run_of(row).shift
    }

    /// Where row `row`'s top is shown, in device pixels from the pane's top (negative above it).
    /// A row past the grid's last is placed as if the grid went on.
    pub fn row_top(&self, row: Row) -> i32 {
        clamp_i32(self.top_of(row.0))
    }

    /// How far row `row` is shown from where it is drawn, in device pixels (up is negative).
    pub fn row_shift(&self, row: Row) -> i32 {
        clamp_i32(self.run_of(row.0).shift)
    }

    /// Where the room under row `after` begins in the pane, when room was made there.
    pub fn insert_top(&self, after: Row) -> Option<i32> {
        self.runs
            .iter()
            .find(|r| r.last == after.0 && r.gap > 0)
            .map(|r| clamp_i32(r.src.1 + r.shift))
    }

    /// What the pane point `at` is over. A point above or below the pane is over what the edge
    /// is over, carried on.
    pub fn to_grid(&self, at: SurfacePoint) -> GridHit {
        let y = i64::from(at.y);
        let drawn = |r: &Run| {
            GridHit::Grid(SurfacePoint {
                x: at.x,
                y: clamp_i32(y - r.shift),
            })
        };
        for r in &self.runs {
            let end = r.src.1 + r.shift;
            if y < end {
                return drawn(r);
            }
            if y < end + r.gap {
                return GridHit::Gap { after: Row(r.last) };
            }
        }
        let last = &self.runs[self.runs.len() - 1];
        match last.gap > 0 {
            true => GridHit::Gap {
                after: Row(last.last),
            },
            false => drawn(last),
        }
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
