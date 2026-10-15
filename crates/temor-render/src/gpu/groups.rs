//! A frame whose own glyphs outgrow the atlases: its rows are built in groups that each fit,
//! and each group keeps the atlases as they stood when its last row was built, to be drawn from
//! in a pass of its own (see `record`).

use std::ops::Range;

use temor_vt::Row;

use super::GpuRenderer;
use super::device::bind;
use super::frame::inst;
use super::pipeline::{Pipeline, texture};
use crate::error::RenderError;
use crate::font::{AtlasCopy, AtlasFormat};
use crate::frame::PaneFrame;
use crate::scene::{SceneInput, build_row};

/// Consecutive grid rows whose glyphs fit the atlases together, and those atlases.
#[derive(Debug)]
pub(super) struct Group {
    pub rows: Range<usize>,
    pub alpha: AtlasCopy,
    pub color: AtlasCopy,
}

impl GpuRenderer {
    /// Stores the instances of the row just built into `self.scene`.
    pub(super) fn keep_row(&mut self, row: Row) {
        if let Some(slot) = self.rows.get_mut(usize::from(row.0)) {
            slot.0 = self.scene.bg.iter().map(inst).collect();
            slot.1 = self.scene.fg.iter().map(inst).collect();
        }
    }

    /// Builds every row of `frame` into `self.rows`, in groups. Fails when a single row's glyphs
    /// do not fit the atlases on their own.
    pub(super) fn build_groups(
        &mut self,
        frame: &PaneFrame,
        input: &SceneInput<'_>,
    ) -> Result<Vec<Group>, RenderError> {
        self.rows = vec![(Vec::new(), Vec::new()); usize::from(frame.size.rows.0)];
        self.glyphs.restart();
        self.glyphs.begin_spill();
        let groups = self.group_rows(frame, input);
        self.glyphs.end_spill();
        // What the device holds is not what is in memory now; the cached rows point into
        // atlases that are gone.
        self.glyphs.forget_uploads();
        self.epoch = u64::MAX;
        groups
    }

    fn group_rows(
        &mut self,
        frame: &PaneFrame,
        input: &SceneInput<'_>,
    ) -> Result<Vec<Group>, RenderError> {
        let total = frame.size.rows.0;
        let mut groups = Vec::new();
        let (mut start, mut at) = (0u16, 0u16);
        while at < total {
            let epoch = self.glyphs.epoch();
            let cells = frame.rows.get(usize::from(at)).map_or(&[][..], |c| c);
            build_row(input, Row(at), cells, &mut self.glyphs, &mut self.scene);
            if self.glyphs.epoch() == epoch {
                self.keep_row(Row(at));
                at += 1;
                continue;
            }
            // An atlas filled up part-way through this row: the rows before it were built into
            // the atlases as they stood just before the clear, and this row starts a group.
            let mut copies = self.glyphs.take_spilled();
            if at == start || copies.len() != 1 {
                return Err(RenderError::GlyphsDoNotFit);
            }
            let (alpha, color) = copies.remove(0);
            groups.push(Group {
                rows: usize::from(start)..usize::from(at),
                alpha,
                color,
            });
            start = at;
            // The failed build left glyphs of this row in the emptied atlases: begin again.
            self.glyphs.restart();
        }
        groups.push(Group {
            rows: usize::from(start)..usize::from(total),
            alpha: self.glyphs.alpha.copy(),
            color: self.glyphs.color.copy(),
        });
        Ok(groups)
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
            view[y * stride..y * stride + row].copy_from_slice(line);
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
