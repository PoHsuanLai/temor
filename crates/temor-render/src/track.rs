//! Which rows a render must compose: each row has a key (when its cells were written, and the
//! cursor, selection and images on it), and the rows whose key differs from the last draw's are
//! composed again, or everything when the view itself changed. The last draw is kept by the
//! renderer once the draw succeeded, so a failed one is planned again in full next time.

use temor_vt::{Col, Generation, GridSize, ImageId, KittyLayer, Px, Row};

use crate::frame::{ImagePlacement, TermColors};
use crate::metrics::{CellMetrics, GridFit, Padding, PixelSize, Scale};
use crate::palette::Palette;
use crate::renderer::{DamageRect, PaneFocus, SelectionOverlay};
use crate::scene::CursorDraw;

/// Everything about the view that, when it changes, invalidates every row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewKey {
    pub size: PixelSize,
    pub scale: Scale,
    pub metrics: CellMetrics,
    pub padding: Padding,
    pub grid: GridSize,
    pub palette: Palette,
    pub colors: TermColors,
    pub focus: PaneFocus,
}

/// A placement's identity and where it lands, in grid rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ImageKey {
    first_row: i32,
    last_row: i32,
    layer: u8,
    id: ImageId,
    generation: Generation,
    col: i32,
    size: (u32, u32),
    source: [u32; 4],
}

/// The rows to compose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Compose {
    All,
    Rows(Vec<Row>),
}

/// What a damage report covers: the whole pane, or only the rows composed this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    Pane,
    Rows,
}

impl Compose {
    pub fn reach(&self) -> Reach {
        match self {
            Compose::All => Reach::Pane,
            Compose::Rows(_) => Reach::Rows,
        }
    }
}

/// What decides how one row looks, apart from the view.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowKey {
    /// When its cells were last written ([`crate::PaneFrame`] numbers every write).
    written: u64,
    cursor: Option<CursorDraw>,
    selected: Option<(Col, Col)>,
    images: Vec<ImageKey>,
}

/// What a draw showed: its view and each row's key.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Drawn {
    view: ViewKey,
    rows: Vec<RowKey>,
}

impl Drawn {
    /// The rows of a frame whose rows were written at `written`, in `view`.
    pub fn of(
        view: ViewKey,
        written: &[u64],
        cursor: Option<CursorDraw>,
        selection: Option<SelectionOverlay>,
        images: &[ImagePlacement],
    ) -> Self {
        let ch = view.metrics.height().max(1) as i32;
        let images: Vec<ImageKey> = images.iter().map(|p| image_key(p, ch)).collect();
        let cols = view.grid.cols.0;
        let rows = (0..view.grid.rows.0)
            .map(|r| {
                let row = Row(r);
                let at = i32::from(r);
                RowKey {
                    written: written.get(usize::from(r)).copied().unwrap_or(0),
                    cursor: cursor.filter(|c| c.row == row),
                    selected: selection.and_then(|s| s.cols_on(row, cols)),
                    images: images
                        .iter()
                        .filter(|k| (k.first_row..=k.last_row).contains(&at))
                        .copied()
                        .collect(),
                }
            })
            .collect();
        Self { view, rows }
    }
}

/// The rows to compose to go from `before` (`None`: nothing is drawn) to `now`.
pub(crate) fn plan(before: Option<&Drawn>, now: &Drawn) -> Compose {
    match before {
        Some(before) if before.view == now.view && before.rows.len() == now.rows.len() => {
            Compose::Rows(
                (0..now.view.grid.rows.0)
                    .map(Row)
                    .filter(|r| before.rows[usize::from(r.0)] != now.rows[usize::from(r.0)])
                    .collect(),
            )
        }
        _ => Compose::All,
    }
}

fn image_key(p: &ImagePlacement, ch: i32) -> ImageKey {
    // Widened: a placement's height is a u32 and may be far taller than the grid.
    let ch = i64::from(ch);
    let span = (i64::from(p.height.0) + ch - 1) / ch;
    let last = i64::from(p.row) + span.max(1) - 1;
    ImageKey {
        first_row: p.row,
        last_row: i32::try_from(last).unwrap_or(i32::MAX),
        layer: match p.layer {
            KittyLayer::BelowBackground => 0,
            KittyLayer::BelowText => 1,
            KittyLayer::AboveText | KittyLayer::All => 2,
        },
        id: p.image.id,
        generation: p.image.generation,
        col: p.col,
        size: (p.width.0, p.height.0),
        source: p.source,
    }
}

/// The device rows `start..end` a grid row's band covers. The first row's band reaches up to the
/// pane's top edge and the last row's down to its bottom edge, so a glyph that overhangs the
/// grid into the outer padding is drawn there. `top` is the grid's top in the pane.
pub(crate) fn band_of(row: Row, rows: u16, top: i32, ch: i32, pane_height: i32) -> (i32, i32) {
    let y = top + i32::from(row.0) * ch;
    let start = if row.0 == 0 { 0.min(y) } else { y };
    let end = if u32::from(row.0) + 1 >= u32::from(rows) {
        pane_height.max(y + ch)
    } else {
        y + ch
    };
    (start, end)
}

/// The damaged rectangles: the whole pane, or each run of adjacent composed rows as a
/// full-width band clipped to the pane. `grid_rows` is the number of rows the frame holds; the
/// first and last band take in the padding beyond them (see [`band_of`]).
pub(crate) fn damage(
    reach: Reach,
    rows: &[Row],
    m: &CellMetrics,
    fit: &GridFit,
    grid_rows: u16,
) -> Vec<DamageRect> {
    let size = fit.pane;
    if reach == Reach::Pane {
        return vec![DamageRect {
            x: Px(0),
            y: Px(0),
            width: size.width,
            height: size.height,
        }];
    }
    let mut sorted: Vec<u16> = rows.iter().map(|r| r.0).collect();
    sorted.sort_unstable();
    sorted.dedup();
    let mut out: Vec<DamageRect> = Vec::new();
    let ch = i32::try_from(m.height()).unwrap_or(i32::MAX);
    let top = i32::try_from(fit.padding.top.0).unwrap_or(i32::MAX);
    let pane = i32::try_from(size.height.0).unwrap_or(i32::MAX);
    for r in sorted {
        let (start, end) = band_of(Row(r), grid_rows, top, ch, pane);
        let (start, end) = (start.max(0), end.min(pane));
        if start >= end {
            continue;
        }
        let (y, height) = (start as u32, (end - start) as u32);
        match out.last_mut() {
            Some(last) if last.y.0 + last.height.0 == y => last.height.0 += height,
            _ => out.push(DamageRect {
                x: Px(0),
                y: Px(y),
                width: size.width,
                height: Px(height),
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests;
