//! Fitting whole cells into a pane and mapping between pixels and cells.

use temor_vt::{Col, GridSize, MouseSurface, Px, Row, SurfacePoint};

use super::model::{
    CellHit, CellMetrics, CellPos, CellSide, GridFit, HitRegion, Padding, PixelSize,
};

/// The most cells a side may hold (the terminal's limit is `u16`).
const MAX_CELLS: u32 = u16::MAX as u32;

/// Fits whole cells of `metrics` into `pane` inside `min_padding`. The grid is at least 1x1
/// (drawn clipped when the pane is smaller than one cell); leftover pixels widen the right and
/// bottom padding so the grid's origin never moves with the pane's size.
pub fn fit_grid(pane: PixelSize, metrics: &CellMetrics, min_padding: Padding) -> GridFit {
    let (cw, ch) = (metrics.width().max(1), metrics.height().max(1));
    let avail_w = pane
        .width
        .0
        .saturating_sub(min_padding.left.0 + min_padding.right.0);
    let avail_h = pane
        .height
        .0
        .saturating_sub(min_padding.top.0 + min_padding.bottom.0);
    let cols = (avail_w / cw).clamp(1, MAX_CELLS);
    let rows = (avail_h / ch).clamp(1, MAX_CELLS);
    let padding = Padding {
        top: min_padding.top,
        left: min_padding.left,
        right: Px(pane.width.0.saturating_sub(min_padding.left.0 + cols * cw)),
        bottom: Px(pane.height.0.saturating_sub(min_padding.top.0 + rows * ch)),
    };
    GridFit {
        pane,
        grid: GridSize::new(cols as u16, rows as u16),
        cell: metrics.cell,
        padding,
    }
}

impl GridFit {
    /// The geometry libghostty's mouse encoder needs to turn pixels into cells.
    pub fn mouse_surface(&self) -> MouseSurface {
        MouseSurface {
            width: self.pane.width,
            height: self.pane.height,
            cell_width: self.cell.width,
            cell_height: self.cell.height,
            padding: self.padding.sides(),
        }
    }

    /// The top-left device pixel of `pos`.
    pub fn cell_origin(&self, pos: CellPos) -> (Px, Px) {
        (
            Px(self.padding.left.0 + u32::from(pos.col.0) * self.cell.width.0),
            Px(self.padding.top.0 + u32::from(pos.row.0) * self.cell.height.0),
        )
    }

    /// The cell under `point` (device pixels from the pane's top-left), clamped to the grid.
    pub fn cell_at(&self, point: SurfacePoint) -> CellHit {
        let (cw, ch) = (
            i64::from(self.cell.width.0.max(1)),
            i64::from(self.cell.height.0.max(1)),
        );
        let x = i64::from(point.x) - i64::from(self.padding.left.0);
        let y = i64::from(point.y) - i64::from(self.padding.top.0);
        let (cols, rows) = (i64::from(self.grid.cols.0), i64::from(self.grid.rows.0));
        let inside = x >= 0 && y >= 0 && x < cols * cw && y < rows * ch;
        let col = x.div_euclid(cw).clamp(0, cols - 1);
        let row = y.div_euclid(ch).clamp(0, rows - 1);
        let within = x - col * cw;
        CellHit {
            pos: CellPos {
                col: Col(col as u16),
                row: Row(row as u16),
            },
            side: if within * 2 < cw {
                CellSide::Left
            } else {
                CellSide::Right
            },
            region: if inside {
                HitRegion::Grid
            } else {
                HitRegion::Padding
            },
        }
    }
}
