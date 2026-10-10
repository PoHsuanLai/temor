//! `CpuRenderer`: the frame loop, the change plan and the output.

use temor_vt::Row;

use super::compose::compose_row;
use super::pixels::Target;
use crate::bands::STRIPE_LOGICAL;
use crate::error::RenderError;
use crate::font::{FontSet, GlyphCache};
use crate::frame::PaneFrame;
use crate::metrics::{CellMetrics, GridFit, PixelSize, Scale, fit_grid};
use crate::renderer::{DamageRect, PaneView, RenderStats, Renderer};
use crate::scene::{Premul, RowScene, SceneInput, resolve_cursor, spans};
use crate::track::{ChangeTracker, Compose, ViewKey, damage};

/// The largest pane side the CPU path draws.
const MAX_SIDE: u32 = 16_384;

/// Composes frames on the calling thread into memory it owns. Upload `pixels` (format
/// `Rgba8Unorm`, premultiplied, rows `size.width * 4` bytes) or just its `damage` rectangles.
#[derive(Debug)]
pub struct CpuRenderer {
    glyphs: GlyphCache,
    buf: Vec<u8>,
    size: PixelSize,
    damage: Vec<DamageRect>,
    tracker: ChangeTracker,
    scene: RowScene,
}

/// One rendered frame.
#[derive(Debug, Clone, Copy)]
pub struct CpuFrame<'a> {
    /// The whole pane, premultiplied RGBA8.
    pub pixels: &'a [u8],
    pub size: PixelSize,
    /// What changed since the previous frame; empty when nothing did.
    pub damage: &'a [DamageRect],
    /// The grid the pane holds at this size (resize the terminal to `fit.grid` and
    /// `fit.cell`).
    pub fit: GridFit,
    pub stats: RenderStats,
}

impl CpuRenderer {
    pub fn new(fonts: FontSet) -> Self {
        Self::from_glyphs(GlyphCache::new(fonts, Scale::default()))
    }

    fn from_glyphs(glyphs: GlyphCache) -> Self {
        Self {
            glyphs,
            buf: Vec::new(),
            size: PixelSize::new(0, 0),
            damage: Vec::new(),
            tracker: ChangeTracker::default(),
            scene: RowScene::default(),
        }
    }

    /// A renderer whose glyph atlases stop growing at the given heights, so tests can force
    /// atlas clears.
    #[cfg(test)]
    pub(crate) fn with_atlas_caps(fonts: FontSet, alpha_max: u32, color_max: u32) -> Self {
        Self::from_glyphs(GlyphCache::with_max_heights(
            fonts,
            Scale::default(),
            alpha_max,
            color_max,
        ))
    }
}

impl Renderer for CpuRenderer {
    type Output<'a> = CpuFrame<'a>;

    fn cell_metrics(&mut self, scale: Scale) -> CellMetrics {
        self.glyphs.set_scale(scale);
        self.glyphs.metrics()
    }

    fn render(
        &mut self,
        frame: &mut PaneFrame,
        view: &PaneView<'_>,
    ) -> Result<CpuFrame<'_>, RenderError> {
        let (w, h) = (view.size.width.0, view.size.height.0);
        if w > MAX_SIDE || h > MAX_SIDE {
            return Err(RenderError::PaneTooLarge {
                width: w,
                height: h,
            });
        }
        let resets = self.glyphs.epoch();
        self.glyphs.set_scale(view.scale);
        let metrics = self.glyphs.metrics();
        let fit = fit_grid(view.size, &metrics, view.padding);
        if view.size != self.size {
            self.size = view.size;
            self.buf = vec![0; (w * h * 4) as usize];
            self.tracker.invalidate();
        }
        let cursor = resolve_cursor(
            frame.cursor,
            |r: Row| frame.rows.get(usize::from(r.0)).map(|c| spans(c)),
            view.focus,
            view.palette,
            &frame.colors,
        );
        let key = ViewKey {
            size: view.size,
            scale: view.scale,
            metrics,
            padding: fit.padding,
            grid: frame.size,
            palette: *view.palette,
            colors: frame.colors,
            focus: view.focus,
            selection: view.selection,
        };
        let plan = self
            .tracker
            .plan(key, frame.take_dirty(), cursor, &frame.images, view.bands);
        let input = SceneInput {
            metrics,
            origin: (fit.padding.left.0 as i32, fit.padding.top.0 as i32),
            cols: frame.size.cols.0,
            palette: view.palette,
            colors: &frame.colors,
            cursor,
            selection: view.selection,
            bands: view.bands,
            pane_width: w,
            stripe_width: view.scale.device(STRIPE_LOGICAL).0.max(1),
        };
        let mut target = Target {
            data: &mut self.buf,
            width: w,
            height: h,
            band: (0, h as i32),
        };
        let reach = plan.reach();
        let rows: Vec<Row> = match plan {
            Compose::All => {
                target.clear_band(Premul::opaque(frame.colors.background));
                (0..frame.size.rows.0).map(Row).collect()
            }
            Compose::Rows(rows) => rows,
        };
        for &row in &rows {
            let cells = frame.rows.get(usize::from(row.0)).map_or(&[][..], |c| c);
            let composed = compose_row(
                &mut target,
                &input,
                row,
                cells,
                &frame.images,
                &mut self.glyphs,
                &mut self.scene,
            );
            if let Err(e) = composed {
                // The plan already took the frame's dirty rows: the next frame draws everything.
                self.tracker.invalidate();
                return Err(e);
            }
        }
        self.glyphs.forget_uploads();
        self.damage = damage(reach, &rows, &metrics, &fit);
        Ok(CpuFrame {
            pixels: &self.buf,
            size: self.size,
            damage: &self.damage,
            fit,
            stats: RenderStats {
                rows_composed: rows.len() as u32,
                atlas_resets: (self.glyphs.epoch() - resets) as u32,
                images_skipped: 0,
            },
        })
    }
}
