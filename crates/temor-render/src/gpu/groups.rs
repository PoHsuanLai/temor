//! Putting a frame's glyphs in the atlases: all at once when they fit, after emptying the
//! atlases when what earlier frames left is in the way, else in groups of rows that each fit,
//! each group keeping the atlases as they stood for it, to be drawn from in a pass of its own
//! (see `record`).

use std::ops::Range;

use super::GpuRenderer;
use super::device::bind;
use super::frame::insts;
use super::pipeline::{Inst, Pipeline, texture};
use crate::error::RenderError;
use crate::font::{AtlasCopy, AtlasFormat, Overflow};

/// The text's instances of consecutive grid rows whose glyphs fit the atlases together, and
/// those atlases.
#[derive(Debug)]
pub(super) struct Group {
    pub fg: Vec<Inst>,
    pub alpha: AtlasCopy,
    pub color: AtlasCopy,
}

impl GpuRenderer {
    /// Puts every glyph of the frame's rows in the atlases, emptying them first when they do
    /// not fit beside what is there. Fails when they do not fit together at all.
    pub(super) fn place(&mut self) -> Result<(), Overflow> {
        let rows = &self.rows;
        let glyphs = || rows.iter().flat_map(|row| row.glyphs());
        if self.atlas.resolve(glyphs(), &mut self.glyphs).is_ok() {
            return Ok(());
        }
        self.atlas.clear();
        self.atlas.resolve(glyphs(), &mut self.glyphs)
    }

    /// The frame's rows in groups whose glyphs fit the atlases. Fails when a single row's
    /// glyphs do not fit on their own.
    pub(super) fn build_groups(&mut self) -> Result<Vec<Group>, RenderError> {
        let mut groups = Vec::new();
        let mut start = 0;
        self.atlas.clear();
        for at in 0..self.rows.len() {
            let glyphs = || self.rows[at].glyphs();
            if self.atlas.resolve(glyphs(), &mut self.glyphs).is_ok() {
                continue;
            }
            // This row's glyphs do not fit beside the group's: the group ends before it, and
            // the row starts the next in emptied atlases.
            if at == start {
                return Err(RenderError::GlyphsDoNotFit);
            }
            groups.push(self.group(start..at));
            start = at;
            self.atlas.clear();
            if self.atlas.resolve(glyphs(), &mut self.glyphs).is_err() {
                return Err(RenderError::GlyphsDoNotFit);
            }
        }
        groups.push(self.group(start..self.rows.len()));
        Ok(groups)
    }

    /// `rows` as a group, with the atlases as they are now.
    fn group(&self, rows: Range<usize>) -> Group {
        let quads = self.rows[rows.clone()].iter().flat_map(|row| &row.fg);
        let (alpha, color) = self.atlas.copy();
        Group {
            fg: insts(quads, &self.atlas).collect(),
            alpha,
            color,
        }
    }
}

/// A texture holding `copy`, filled by a copy recorded in `enc`, so it lands in order with the
/// passes (`queue.write_texture` would land before every one of them).
fn upload(
    device: &wgpu::Device,
    enc: &mut wgpu::CommandEncoder,
    copy: &AtlasCopy,
) -> wgpu::Texture {
    let format = match copy.format {
        AtlasFormat::Alpha => wgpu::TextureFormat::R8Unorm,
        AtlasFormat::Color => wgpu::TextureFormat::Rgba8Unorm,
    };
    let tex = texture(
        device,
        "temor atlas group",
        (copy.width, copy.height),
        format,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let row = (copy.width * copy.format.bytes_per_texel()) as usize;
    let stride = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("temor atlas group staging"),
        size: (stride * copy.height as usize) as u64,
        usage: wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: true,
    });
    {
        let mut view = staging.get_mapped_range_mut(..);
        for (y, line) in copy.data.chunks_exact(row).enumerate() {
            view.slice(y * stride..y * stride + row)
                .copy_from_slice(line);
        }
    }
    staging.unmap();
    enc.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride as u32),
                rows_per_image: Some(copy.height),
            },
        },
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width: copy.width,
            height: copy.height,
            depth_or_array_layers: 1,
        },
    );
    tex
}

/// The bind group that reads `group`'s atlases, uploaded in `enc`. The textures live as long as
/// the bind group; the command buffer keeps them until it has run.
pub(super) fn bind_group(
    device: &wgpu::Device,
    pipe: &Pipeline,
    enc: &mut wgpu::CommandEncoder,
    group: &Group,
) -> wgpu::BindGroup {
    let alpha = upload(device, enc, &group.alpha);
    let color = upload(device, enc, &group.color);
    bind(device, pipe, &alpha, &color)
}
