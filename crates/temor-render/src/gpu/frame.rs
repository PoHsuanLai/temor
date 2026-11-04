//! `GpuRenderer`'s render: build the composed rows' quads, put the glyphs the frame shows in
//! the atlases (clearing them, or drawing in groups, when they do not fit), record the pass.

use temor_vt::Row;

use super::pipeline::{ALPHA, COLOR, IMAGE, Inst, SOLID, texture};
use super::{GpuFrame, GpuRenderer};
use crate::error::RenderError;
use crate::font::{Atlases, Overflow};
use crate::frame::{ImagePlacement, PaneFrame};
use crate::metrics::{CellMetrics, Scale, fit_grid};
use crate::renderer::{PaneView, RenderStats, Renderer};
use crate::scene::{Quad, QuadPaint, RowScene, SceneInput, build_row, resolve_cursor, spans};
use crate::track::{Compose, Drawn, ViewKey, damage, plan};

/// `q`'s instance, its glyph where `atlas` has put it; none for a glyph it has not.
pub(super) fn inst(q: &Quad, atlas: &Atlases) -> Option<Inst> {
    let rect = [q.x as f32, q.y as f32, q.w as f32, q.h as f32];
    let f = |c: [u8; 4]| c.map(|v| f32::from(v) / 255.0);
    let uv = |glyph| {
        let s = atlas.slot(glyph)?;
        Some([
            s.x as f32,
            s.y as f32,
            (s.x + s.w) as f32,
            (s.y + s.h) as f32,
        ])
    };
    let (uv, color, kind) = match q.paint {
        QuadPaint::Solid(c) => ([0.0; 4], f(c.0), SOLID),
        QuadPaint::Mask { glyph, color } => (uv(glyph)?, f(color.0), ALPHA),
        QuadPaint::Color { glyph } => (uv(glyph)?, [1.0; 4], COLOR),
    };
    Some(Inst {
        rect,
        uv,
        color,
        kind,
        _pad: [0; 3],
    })
}

/// The instances of `quads`, their glyphs where `atlas` has put them.
pub(super) fn insts<'q>(
    quads: impl IntoIterator<Item = &'q Quad>,
    atlas: &Atlases,
) -> impl Iterator<Item = Inst> {
    quads.into_iter().filter_map(|q| inst(q, atlas))
}

pub(super) fn image_inst(p: &ImagePlacement, origin: (i32, i32), m: &CellMetrics) -> Inst {
    let (iw, ih) = (p.image.width.max(1) as f32, p.image.height.max(1) as f32);
    let [sx, sy, sw, sh] = p.source.map(|v| v as f32);
    Inst {
        rect: [
            (origin.0 + p.col * m.width() as i32 + p.offset.0 as i32) as f32,
            (origin.1 + p.row * m.height() as i32 + p.offset.1 as i32) as f32,
            p.width.0 as f32,
            p.height.0 as f32,
        ],
        uv: [sx / iw, sy / ih, (sx + sw) / iw, (sy + sh) / ih],
        color: [1.0; 4],
        kind: IMAGE,
        _pad: [0; 3],
    }
}

impl Renderer for GpuRenderer {
    type Output<'a> = GpuFrame<'a>;

    fn cell_metrics(&self, scale: Scale) -> CellMetrics {
        self.glyphs.metrics_at(scale)
    }

    fn render(
        &mut self,
        frame: &PaneFrame,
        view: &PaneView<'_>,
    ) -> Result<GpuFrame<'_>, RenderError> {
        let (w, h) = (view.size.width.0.max(1), view.size.height.0.max(1));
        let max = self.device.limits().max_texture_dimension_2d;
        if w > max || h > max {
            return Err(RenderError::PaneTooLarge {
                width: w,
                height: h,
            });
        }
        let clears = self.atlas.clears();
        self.glyphs.trim();
        let metrics = self.metrics(view.scale);
        let fit = fit_grid(view.size, &metrics, view.padding).lowered(view.lower);
        let colors = view.colors(frame);
        if view.size != self.size || self.target.is_none() {
            self.size = view.size;
            let usage = wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC;
            self.target = Some(texture(
                &self.device,
                "temor pane",
                (w, h),
                wgpu::TextureFormat::Rgba8Unorm,
                usage,
            ));
            let size = [w as f32, h as f32, 0.0, 0.0];
            self.queue
                .write_buffer(&self.pipe.uniform, 0, bytemuck::cast_slice(&size));
            self.drawn = None;
        }
        let cursor = resolve_cursor(
            frame.cursor,
            |r: Row| frame.rows.get(usize::from(r.0)).map(|c| spans(c)),
            (view.focus, view.blink),
            view.palette,
            &colors,
        );
        let key = ViewKey {
            size: view.size,
            scale: view.scale,
            metrics,
            padding: fit.padding,
            grid: frame.size,
            palette: *view.palette,
            colors,
            focus: view.focus,
        };
        let now = Drawn::of(key, &frame.written, cursor, view.selection, &frame.images);
        let plan = plan(self.drawn.as_ref(), &now);
        let all: Vec<Row> = (0..frame.size.rows.0).map(Row).collect();
        let reach = plan.reach();
        let mut rows = match plan {
            Compose::All => all.clone(),
            Compose::Rows(rows) => rows,
        };
        if self.rows.len() != all.len() {
            self.rows = vec![RowScene::default(); all.len()];
            rows.clone_from(&all);
        }
        let input = SceneInput {
            metrics,
            origin: (fit.padding.left.0 as i32, fit.padding.top.0 as i32),
            cols: frame.size.cols.0,
            palette: view.palette,
            colors: &colors,
            cursor,
            selection: view.selection,
        };
        for &row in &rows {
            let at = usize::from(row.0);
            let cells = frame.rows.get(at).map_or(&[][..], |c| c);
            if let Some(scene) = self.rows.get_mut(at) {
                build_row(&input, row, cells, &mut self.glyphs, scene);
            }
        }
        // The frame's glyphs do not fit the atlases together: it is drawn in groups of rows that
        // do, each with the atlases as they were for it.
        let groups = match self.place() {
            Ok(()) => None,
            // A failed draw leaves the last one standing: the next plans from it again.
            Err(Overflow) => Some(self.build_groups()?),
        };
        let (commands, images_skipped) = self.record(frame, &input, groups.as_deref())?;
        self.drawn = Some(now);
        self.damage = damage(reach, &rows, &metrics, &fit, frame.size.rows.0);
        let texture = self.target.as_ref().ok_or(RenderError::NoRenderTarget)?;
        Ok(GpuFrame {
            texture,
            commands,
            damage: &self.damage,
            fit,
            stats: RenderStats {
                rows_composed: rows.len() as u32,
                atlas_resets: (self.atlas.clears() - clears) as u32,
                images_skipped,
            },
        })
    }
}
