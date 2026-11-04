//! Recording a frame's passes: images and instances uploaded, then either one pass over every
//! row, or (when the frame's glyphs outgrow the atlases) one for everything under the text and
//! one per group of rows, each reading its own atlases.

use std::ops::Range;

use temor_vt::{ImageId, KittyLayer};

use super::GpuRenderer;
use super::device::ensure_image;
use super::frame::{image_inst, insts};
use super::groups::{Group, bind_group};
use super::pipeline::Inst;
use crate::error::RenderError;
use crate::frame::PaneFrame;
use crate::scene::SceneInput;

/// A kitty image to draw: its layer, id and instance index.
type Draw = (KittyLayer, ImageId, u32);

/// Where the images and the instances of each kind sit in the instance buffer.
struct Layout {
    draws: Vec<Draw>,
    /// Backgrounds of every row.
    bg: Range<u32>,
    /// The text of each span of rows, in order.
    fg: Vec<Range<u32>>,
}

impl GpuRenderer {
    /// Uploads images and instances and records the passes. With `groups`, the text is drawn
    /// group by group, each with its own atlases.
    pub(super) fn record(
        &mut self,
        frame: &PaneFrame,
        input: &SceneInput<'_>,
        groups: Option<&[Group]>,
    ) -> Result<(wgpu::CommandBuffer, u32), RenderError> {
        let (origin, m) = (input.origin, &input.metrics);
        let mut all: Vec<Inst> =
            insts(self.rows.iter().flat_map(|row| &row.bg), &self.atlas).collect();
        let bg = 0..all.len() as u32;
        let mut fg = Vec::new();
        let mut text = |all: &mut Vec<Inst>, insts: &mut dyn Iterator<Item = Inst>| {
            let from = all.len() as u32;
            all.extend(insts);
            fg.push(from..all.len() as u32);
        };
        match groups {
            Some(groups) => {
                for group in groups {
                    text(&mut all, &mut group.fg.iter().copied());
                }
            }
            None => {
                let quads = self.rows.iter().flat_map(|row| &row.fg);
                text(&mut all, &mut insts(quads, &self.atlas));
            }
        }
        let mut draws: Vec<Draw> = Vec::new();
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
        self.on_device
            .sync(&self.device, &self.queue, &self.pipe, &mut self.atlas);
        self.upload_instances(&all)?;
        let layout = Layout { draws, bg, fg };
        let commands = self.encode(input.colors.background, &layout, groups)?;
        Ok((commands, images_skipped))
    }

    fn upload_instances(&mut self, all: &[Inst]) -> Result<(), RenderError> {
        let bytes: &[u8] = bytemuck::cast_slice(all);
        let need = (bytes.len() as u64).max(64).next_power_of_two();
        if self.instances.as_ref().is_none_or(|b| b.size() < need) {
            self.instances = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("temor instances"),
                size: need,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let Some(buf) = self.instances.as_ref() else {
            return Err(RenderError::NoRenderTarget);
        };
        if !bytes.is_empty() {
            self.queue.write_buffer(buf, 0, bytes);
        }
        Ok(())
    }

    fn encode(
        &self,
        bg: temor_vt::Rgb,
        layout: &Layout,
        groups: Option<&[Group]>,
    ) -> Result<wgpu::CommandBuffer, RenderError> {
        let (Some(buf), Some(target)) = (self.instances.as_ref(), self.target.as_ref()) else {
            return Err(RenderError::NoRenderTarget);
        };
        let view = target.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        let c = |v: u8| f64::from(v) / 255.0;
        let clear = wgpu::LoadOp::Clear(wgpu::Color {
            r: c(bg.r),
            g: c(bg.g),
            b: c(bg.b),
            a: 1.0,
        });
        let draw = |pass: &mut wgpu::RenderPass<'_>, layer: KittyLayer| {
            for &(l, id, at) in &layout.draws {
                if let (true, Some(img)) = (l == layer, self.images.get(&id)) {
                    pass.set_bind_group(1, &img.bind, &[]);
                    pass.draw(0..4, at..at + 1);
                }
            }
        };
        let text = |pass: &mut wgpu::RenderPass<'_>, range: &Range<u32>| {
            pass.set_bind_group(1, &self.blank, &[]);
            if !range.is_empty() {
                pass.draw(0..4, range.clone());
            }
        };
        let under_text = |pass: &mut wgpu::RenderPass<'_>| {
            pass.set_pipeline(&self.pipe.pipeline);
            pass.set_bind_group(0, &self.on_device.bind, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            draw(pass, KittyLayer::BelowBackground);
            text(pass, &layout.bg);
            draw(pass, KittyLayer::BelowText);
        };
        match groups {
            None => {
                let mut pass = begin(&mut enc, &view, clear);
                under_text(&mut pass);
                for range in &layout.fg {
                    text(&mut pass, range);
                }
                draw(&mut pass, KittyLayer::AboveText);
            }
            Some(groups) => {
                under_text(&mut begin(&mut enc, &view, clear));
                for (group, range) in groups.iter().zip(&layout.fg) {
                    let atlases = bind_group(&self.device, &self.pipe, &mut enc, group);
                    let mut pass = begin(&mut enc, &view, wgpu::LoadOp::Load);
                    pass.set_pipeline(&self.pipe.pipeline);
                    pass.set_bind_group(0, &atlases, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    text(&mut pass, range);
                }
                let mut pass = begin(&mut enc, &view, wgpu::LoadOp::Load);
                pass.set_pipeline(&self.pipe.pipeline);
                pass.set_bind_group(0, &self.on_device.bind, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                draw(&mut pass, KittyLayer::AboveText);
            }
        }
        Ok(enc.finish())
    }
}

/// A render pass over `view` that starts from `load`.
fn begin<'e>(
    enc: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("temor cells"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
