//! `GpuRenderer`: construction, the device state it keeps, and its frame output.

use std::collections::HashMap;

use temor_vt::ImageId;

use super::device::{DeviceAtlases, DeviceImage};
use super::pipeline::{self, Pipeline};
use crate::font::{Atlases, FontSet, GlyphCache};
use crate::metrics::{CellMetrics, GridFit, PixelSize, Scale};
use crate::renderer::{DamageRect, RenderStats};
use crate::scene::RowScene;
use crate::track::Drawn;

/// Renders panes on a wgpu device into its own `Rgba8Unorm` (premultiplied) texture.
#[derive(Debug)]
pub struct GpuRenderer {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) pipe: Pipeline,
    pub(super) glyphs: GlyphCache,
    /// Where the glyphs the frame shows are, in memory.
    pub(super) atlas: Atlases,
    /// The atlases on the device.
    pub(super) on_device: DeviceAtlases,
    pub(super) blank: wgpu::BindGroup,
    pub(super) target: Option<wgpu::Texture>,
    pub(super) size: PixelSize,
    /// Per grid row: its quads, rebuilt only for composed rows.
    pub(super) rows: Vec<RowScene>,
    /// What the last draw that succeeded showed.
    pub(super) drawn: Option<Drawn>,
    pub(super) instances: Option<wgpu::Buffer>,
    pub(super) images: HashMap<ImageId, DeviceImage>,
    pub(super) damage: Vec<DamageRect>,
}

/// One recorded frame. Submit `commands` on the window's render thread (see the module docs),
/// then show `texture`.
#[derive(Debug)]
pub struct GpuFrame<'a> {
    pub texture: &'a wgpu::Texture,
    pub commands: wgpu::CommandBuffer,
    pub damage: &'a [DamageRect],
    pub fit: GridFit,
    pub stats: RenderStats,
}

impl GpuRenderer {
    /// A renderer on `device`, drawing with `fonts`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, fonts: FontSet) -> Self {
        let glyphs = GlyphCache::new(fonts, Scale::default());
        Self::from_parts(device, queue, glyphs, Atlases::default())
    }

    /// A renderer whose glyph atlases stop growing at these heights, so a test can make a
    /// frame's glyphs outgrow them.
    #[cfg(test)]
    pub(crate) fn with_atlas_caps(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        fonts: FontSet,
        alpha_max: u32,
        color_max: u32,
    ) -> Self {
        let glyphs = GlyphCache::new(fonts, Scale::default());
        Self::from_parts(
            device,
            queue,
            glyphs,
            Atlases::with_caps(alpha_max, color_max),
        )
    }

    fn from_parts(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        glyphs: GlyphCache,
        atlas: Atlases,
    ) -> Self {
        let pipe = Pipeline::new(device);
        let on_device = DeviceAtlases::new(device, &pipe, &atlas);
        let blank_tex = pipeline::texture(
            device,
            "temor blank",
            (1, 1),
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let blank = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("temor blank"),
            layout: &pipe.layout1,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &blank_tex.create_view(&Default::default()),
                ),
            }],
        });
        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipe,
            glyphs,
            atlas,
            on_device,
            blank,
            target: None,
            size: PixelSize::new(0, 0),
            rows: Vec::new(),
            drawn: None,
            instances: None,
            images: HashMap::new(),
            damage: Vec::new(),
        }
    }

    /// The cell at `scale`, the glyphs moved to it: the atlases held the glyphs of another
    /// scale under the same names, and are emptied.
    pub(super) fn metrics(&mut self, scale: Scale) -> CellMetrics {
        if self.glyphs.set_scale(scale) {
            self.atlas.clear();
        }
        self.glyphs.metrics()
    }
}
