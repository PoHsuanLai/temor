//! Placing the images of unicode placeholders: each run of placeholder cells shows its slice of
//! the image, fitted into the placement's grid the way libghostty's own renderer fits it
//! (`Placement.renderPlacement` in `graphics_unicode.zig`).

// Read only by `FrameCapture` (feature `ghostty`) and tests.
#![cfg_attr(not(feature = "ghostty"), allow(dead_code))]

use std::sync::Arc;

use temor_vt::{KittyLayer, PlacementId, Px};

use super::model::{ImagePixels, ImagePlacement};
use super::placeholder::{PlaceholderCell, PlaceholderRun, runs};

/// A virtual placement (`U=1`): the image and the grid of cells it is spread over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VirtualTarget {
    pub layer: KittyLayer,
    pub image: Arc<ImagePixels>,
    /// `PlacementId(0)` when the program gave none.
    pub placement: PlacementId,
    /// The grid libghostty reports (`Placement.gridSize`): the program's own `c=`/`r=` when
    /// given, and when only one is, the other computed from the image's aspect ratio, as
    /// kitty's protocol says. Nothing is left to compute here.
    pub cols: u32,
    pub rows: u32,
}

/// What a capture read about virtual placements, to meet the placeholder cells with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct VirtualScene {
    pub targets: Vec<VirtualTarget>,
    /// One cell in pixels, as the terminal was told.
    pub cell: (u32, u32),
}

impl VirtualScene {
    /// The placement a run names: the exact one when it gives a placement id, else the image's
    /// first. libghostty does not say whether a placement id is the program's or one it made up
    /// for a placement given none (its C API hides the tag), so a made-up id N can be matched
    /// by a run that names N.
    fn target(&self, run: &PlaceholderRun) -> Option<&VirtualTarget> {
        self.targets.iter().find(|t| {
            t.image.id == run.image
                && (run.placement == PlacementId(0) || t.placement == run.placement)
        })
    }
}

/// The images the placeholder cells of every row stand for, one per run.
pub(crate) fn virtual_images(
    rows: &[Vec<PlaceholderCell>],
    scene: &VirtualScene,
) -> Vec<ImagePlacement> {
    let mut out = Vec::new();
    for (y, cells) in rows.iter().enumerate() {
        for run in runs(cells) {
            let Some(target) = scene.target(&run) else {
                continue;
            };
            let image = (target.image.width, target.image.height);
            let Some(fit) = fit(&run, (target.cols, target.rows), image, scene.cell) else {
                continue;
            };
            out.push(ImagePlacement {
                layer: target.layer,
                image: target.image.clone(),
                col: i32::from(run.col.0),
                row: i32::try_from(y).unwrap_or(i32::MAX),
                offset: fit.offset,
                width: Px(fit.size.0),
                height: Px(fit.size.1),
                source: fit.source,
            });
        }
    }
    out
}

/// Where a run lands: pixel offset inside its first cell, the drawn size and the source
/// rectangle in image pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fit {
    pub offset: (u32, u32),
    pub size: (u32, u32),
    pub source: [u32; 4],
}

/// The image scaled to fit the grid (centred, aspect kept), cut to the cells of `run`. `None`
/// when nothing of the image falls in those cells or a size is zero.
pub(crate) fn fit(
    run: &PlaceholderRun,
    grid: (u32, u32),
    image: (u32, u32),
    cell: (u32, u32),
) -> Option<Fit> {
    if [grid.0, grid.1, image.0, image.1, cell.0, cell.1].contains(&0) {
        return None;
    }
    let (cols, rows) = (f64::from(grid.0), f64::from(grid.1));
    let (img_w, img_h) = (f64::from(image.0), f64::from(image.1));
    let (cell_w, cell_h) = (f64::from(cell.0), f64::from(cell.1));
    let (grid_w, grid_h) = (cols * cell_w, rows * cell_h);

    // Fit the whole image into the grid, centred on the side with room to spare.
    let (scale, pad_x, pad_y) = if img_w * grid_h > img_h * grid_w {
        let s = grid_w / img_w;
        (s, 0.0, (grid_h - img_h * s) / 2.0)
    } else {
        let s = grid_h / img_h;
        (s, (grid_w - img_w * s) / 2.0, 0.0)
    };
    // The padding in image pixels, and the padded image's size.
    let (lead_x, lead_y) = (pad_x / scale, pad_y / scale);
    let (full_w, full_h) = (img_w + 2.0 * lead_x, img_h + 2.0 * lead_y);

    // The run's rectangle in the padded image, then cut to the image itself on each axis.
    let mut x = Axis {
        start: full_w * (f64::from(run.column) / cols),
        len: full_w * (f64::from(run.width) / cols),
        dest: f64::from(run.width) * cell_w,
        offset: 0.0,
    };
    let mut y = Axis {
        start: full_h * (f64::from(run.row) / rows),
        len: full_h / rows,
        dest: cell_h,
        offset: 0.0,
    };
    x.cut(Padding {
        lead: lead_x,
        full: full_w,
        image: img_w,
        scale,
    });
    y.cut(Padding {
        lead: lead_y,
        full: full_h,
        image: img_h,
        scale,
    });
    if x.len <= 0.0 || y.len <= 0.0 {
        return None;
    }
    let px = |v: f64| v.round().max(0.0) as u32;
    Some(Fit {
        offset: (px(x.offset * scale), px(y.offset * scale)),
        size: (px(x.dest), px(y.dest)),
        source: [px(x.start), px(y.start), px(x.len), px(y.len)],
    })
}

/// How an axis of the image is padded inside the grid, all in image pixels but `scale`.
#[derive(Clone, Copy)]
struct Padding {
    /// The padding before the image (and after it).
    lead: f64,
    /// The padded size.
    full: f64,
    /// The image's size.
    image: f64,
    /// Device pixels per image pixel.
    scale: f64,
}

/// One axis of a run: its span in the padded image and where it is drawn.
struct Axis {
    start: f64,
    len: f64,
    /// The drawn length in device pixels.
    dest: f64,
    /// Where the drawing starts inside its first cell, in image pixels.
    offset: f64,
}

impl Axis {
    /// Moves the span from padded-image pixels to image pixels. A span that starts in the
    /// padding, or reaches into the padding after the image, is shortened to the image and
    /// drawn that much further in (or shorter).
    fn cut(&mut self, pad: Padding) {
        let Padding {
            lead,
            full,
            image,
            scale,
        } = pad;
        if self.start < lead {
            let into = lead - self.start;
            self.len -= into;
            self.offset = into;
            self.dest -= into * scale;
            self.start = 0.0;
            if self.len > image {
                self.len = image;
                self.dest = image * scale;
            }
        } else if self.start + self.len > full - lead {
            self.start -= lead;
            self.len = full - lead - self.start - lead;
            self.dest = self.len * scale;
        } else {
            self.start -= lead;
        }
    }
}

#[cfg(test)]
mod tests;
