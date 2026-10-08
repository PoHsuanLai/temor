//! Which rows a render must compose: the frame's dirty rows, plus the rows a moved cursor or a
//! changed image touches, or everything when the view itself changed.

use std::collections::BTreeSet;

use temor_vt::{Generation, GridSize, ImageId, KittyLayer, Px, Row};

use crate::frame::ImagePlacement;
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
    pub focus: PaneFocus,
    pub selection: Option<SelectionOverlay>,
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

/// What was drawn last time, to tell what changed.
#[derive(Debug, Default)]
pub(crate) struct ChangeTracker {
    view: Option<ViewKey>,
    cursor: Option<CursorDraw>,
    images: Vec<ImageKey>,
}

impl ChangeTracker {
    /// Forgets everything: the next plan is `All`.
    pub fn invalidate(&mut self) {
        self.view = None;
    }

    pub fn plan(
        &mut self,
        view: ViewKey,
        dirty: Vec<Row>,
        cursor: Option<CursorDraw>,
        images: &[ImagePlacement],
    ) -> Compose {
        let ch = view.metrics.height().max(1) as i32;
        let keys: Vec<ImageKey> = images.iter().map(|p| image_key(p, ch)).collect();
        let full = self.view != Some(view);
        let old_cursor = std::mem::replace(&mut self.cursor, cursor);
        let old_images = std::mem::replace(&mut self.images, keys);
        self.view = Some(view);
        if full {
            return Compose::All;
        }
        let rows = i32::from(view.grid.rows.0);
        let mut set: BTreeSet<u16> = dirty.into_iter().map(|r| r.0).collect();
        if old_cursor != cursor {
            set.extend(old_cursor.iter().chain(cursor.iter()).map(|c| c.row.0));
        }
        let changed = old_images
            .iter()
            .filter(|k| !self.images.contains(k))
            .chain(self.images.iter().filter(|k| !old_images.contains(k)));
        for k in changed {
            for r in k.first_row.max(0)..=k.last_row.min(rows - 1) {
                set.insert(r as u16);
            }
        }
        set.retain(|&r| i32::from(r) < rows);
        Compose::Rows(set.into_iter().map(Row).collect())
    }
}

fn image_key(p: &ImagePlacement, ch: i32) -> ImageKey {
    let span = (p.height.0 as i32 + ch - 1) / ch;
    ImageKey {
        first_row: p.row,
        last_row: p.row + span.max(1) - 1,
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

/// The damaged rectangles: the whole pane, or each run of adjacent composed rows as a
/// full-width band clipped to the pane.
pub(crate) fn damage(full: bool, rows: &[Row], m: &CellMetrics, fit: &GridFit) -> Vec<DamageRect> {
    let size = fit.pane;
    if full {
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
    let ch = m.height();
    for r in sorted {
        let y = fit.padding.top.0 + u32::from(r) * ch;
        if y >= size.height.0 {
            continue;
        }
        let height = ch.min(size.height.0 - y);
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
