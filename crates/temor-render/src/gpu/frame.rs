//! `GpuRenderer`'s render: rebuild the instances of composed rows, sync the atlases, record
//! the pass.

use temor_vt::{KittyLayer, Row};

use super::device::ensure_image;
use super::pipeline::{ALPHA, COLOR, IMAGE, Inst, SOLID, texture};
use super::{GpuFrame, GpuRenderer};
use crate::bands::STRIPE_LOGICAL;
use crate::error::RenderError;
use crate::font::BUILD_ATTEMPTS;
use crate::frame::{ImagePlacement, PaneFrame};
use crate::metrics::{CellMetrics, Scale, fit_grid};
use crate::renderer::{PaneView, RenderStats, Renderer};
use crate::scene::{Quad, QuadPaint, SceneInput, build_row, resolve_cursor, spans};
use crate::track::{Compose, ViewKey, damage};

fn inst(q: &Quad) -> Inst {
    let rect = [q.x as f32, q.y as f32, q.w as f32, q.h as f32];
    let f = |c: [u8; 4]| c.map(|v| f32::from(v) / 255.0);
    let uv = |s: crate::font::Slot| {
        [
            s.x as f32,
            s.y as f32,
            (s.x + s.w) as f32,
            (s.y + s.h) as f32,
        ]
    };
    let (uv, color, kind) = match q.paint {
        QuadPaint::Solid(c) => ([0.0; 4], f(c.0), SOLID),
        QuadPaint::Mask { slot, color } => (uv(slot), f(color.0), ALPHA),
        QuadPaint::Color { slot } => (uv(slot), [1.0; 4], COLOR),
    };
    Inst {
        rect,
        uv,
        color,
        kind,
        _pad: [0; 3],
    }
}

fn image_inst(p: &ImagePlacement, origin: (i32, i32), m: &CellMetrics) -> Inst {
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

    fn cell_metrics(&mut self, scale: Scale) -> CellMetrics {
        self.metrics(scale)
    }

    fn render(
        &mut self,
        frame: &mut PaneFrame,
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
        let resets = self.glyphs.epoch();
        let metrics = self.metrics(view.scale);
        let fit = fit_grid(view.size, &metrics, view.padding).lowered(view.lower);
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
            self.tracker.invalidate();
        }
        let cursor = resolve_cursor(
            frame.cursor,
            |r: Row| frame.rows.get(usize::from(r.0)).map(|c| spans(c)),
            (view.focus, view.blink),
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
        };
        let plan = self.tracker.plan(
            key,
            frame.take_dirty(),
            cursor,
            view.selection,
            &frame.images,
            view.bands,
        );
        let all: Vec<Row> = (0..frame.size.rows.0).map(Row).collect();
        let reach = plan.reach();
        let mut rows = match plan {
            Compose::All => all.clone(),
            Compose::Rows(rows) => rows,
        };
        if self.rows.len() != all.len() || self.epoch != self.glyphs.epoch() {
            self.rows = vec![(Vec::new(), Vec::new()); all.len()];
            rows.clone_from(&all);
        }
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
        // An atlas clear while building invalidates the glyphs of rows built before it: build
        // again, every row this time, until a build finishes without a clear.
        let mut settled = false;
        for attempt in 0..BUILD_ATTEMPTS {
            let before = self.glyphs.epoch();
            let todo = if attempt == 0 { &rows } else { &all };
            for &row in todo {
                let cells = frame.rows.get(usize::from(row.0)).map_or(&[][..], |c| c);
                build_row(&input, row, cells, &mut self.glyphs, &mut self.scene);
                if let Some(slot) = self.rows.get_mut(usize::from(row.0)) {
                    slot.0 = self.scene.bg.iter().map(inst).collect();
                    slot.1 = self.scene.fg.iter().map(inst).collect();
                }
            }
            if self.glyphs.epoch() == before {
                self.epoch = before;
                settled = true;
                break;
            }
        }
        if !settled {
            // The plan took the dirty rows: the next frame draws everything.
            self.tracker.invalidate();
            return Err(RenderError::GlyphsDoNotFit);
        }
        let (commands, images_skipped) = self.record(frame, input.origin, &metrics)?;
        self.damage = damage(reach, &rows, &metrics, &fit);
        let texture = self.target.as_ref().ok_or(RenderError::NoRenderTarget)?;
        Ok(GpuFrame {
            texture,
            commands,
            damage: &self.damage,
            fit,
            stats: RenderStats {
                rows_composed: rows.len() as u32,
                atlas_resets: (self.glyphs.epoch() - resets) as u32,
                images_skipped,
            },
        })
    }
}

impl GpuRenderer {
    /// Uploads atlases, images and instances, and records the pass.
    fn record(
        &mut self,
        frame: &PaneFrame,
        origin: (i32, i32),
        m: &CellMetrics,
    ) -> Result<(wgpu::CommandBuffer, u32), RenderError> {
        let mut all: Vec<Inst> = self.rows.iter().flat_map(|r| r.0.iter().copied()).collect();
        let bg_end = all.len() as u32;
        all.extend(self.rows.iter().flat_map(|r| r.1.iter().copied()));
        let fg_end = all.len() as u32;
        let mut draws: Vec<(KittyLayer, temor_vt::ImageId, u32)> = Vec::new();
        let mut images_skipped = 0;
        let limit = self.device.limits().max_texture_dimension_2d;
        for p in &frame.images {
            let uploaded = ensure_image(
                &mut self.images,
                &self.device,
                &self.queue,
                &self.pipe,
                p,
                limit,
            );
            if uploaded.is_err() {
                // Not drawn; counted in the stats.
                images_skipped += 1;
                continue;
            }
            draws.push((p.layer, p.image.id, all.len() as u32));
            all.push(image_inst(p, origin, m));
        }
        self.images
            .retain(|id, _| frame.images.iter().any(|p| p.image.id == *id));
        self.atlases
            .sync(&self.device, &self.queue, &self.pipe, &mut self.glyphs);
        let bytes: &[u8] = bytemuck::cast_slice(&all);
        let need = (bytes.len() as u64).max(64).next_power_of_two();
        if self.instances.as_ref().is_none_or(|b| b.size() < need) {
            self.instances = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("temor instances"),
                size: need,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let (Some(buf), Some(target)) = (self.instances.as_ref(), self.target.as_ref()) else {
            return Err(RenderError::NoRenderTarget);
        };
        if !bytes.is_empty() {
            self.queue.write_buffer(buf, 0, bytes);
        }
        let target_view = target.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let bg = frame.colors.background;
            let c = |v: u8| f64::from(v) / 255.0;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("temor cells"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: c(bg.r),
                            g: c(bg.g),
                            b: c(bg.b),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe.pipeline);
            pass.set_bind_group(0, &self.atlases.bind, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            let images = &self.images;
            let draw_layer = |pass: &mut wgpu::RenderPass<'_>, layer: KittyLayer| {
                for &(l, id, at) in &draws {
                    if let (true, Some(img)) = (l == layer, images.get(&id)) {
                        pass.set_bind_group(1, &img.bind, &[]);
                        pass.draw(0..4, at..at + 1);
                    }
                }
            };
            draw_layer(&mut pass, KittyLayer::BelowBackground);
            pass.set_bind_group(1, &self.blank, &[]);
            if bg_end > 0 {
                pass.draw(0..4, 0..bg_end);
            }
            draw_layer(&mut pass, KittyLayer::BelowText);
            pass.set_bind_group(1, &self.blank, &[]);
            if fg_end > bg_end {
                pass.draw(0..4, bg_end..fg_end);
            }
            draw_layer(&mut pass, KittyLayer::AboveText);
        }
        Ok((enc.finish(), images_skipped))
    }
}
