//! `GpuRenderer`: construction, the device state it keeps, and its frame output.

use std::collections::HashMap;

use temor_vt::ImageId;

use super::device::{DeviceAtlases, DeviceImage};
use super::pipeline::{self, Inst, Pipeline};
use crate::font::{FontSet, GlyphCache};
use crate::metrics::{CellMetrics, GridFit, PixelSize, Scale};
use crate::renderer::{DamageRect, RenderStats};
use crate::scene::RowScene;
use crate::track::ChangeTracker;

/// Renders panes on a wgpu device into its own `Rgba8Unorm` (premultiplied) texture.
#[derive(Debug)]
pub struct GpuRenderer {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) pipe: Pipeline,
    pub(super) glyphs: GlyphCache,
    pub(super) atlases: DeviceAtlases,
    pub(super) blank: wgpu::BindGroup,
    pub(super) target: Option<wgpu::Texture>,
    pub(super) size: PixelSize,
    /// Per grid row: background and foreground instances, rebuilt only for composed rows.
    pub(super) rows: Vec<(Vec<Inst>, Vec<Inst>)>,
    /// The glyph-cache epoch the cached rows were built in.
    pub(super) epoch: u64,
    pub(super) tracker: ChangeTracker,
    pub(super) scene: RowScene,
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
        let pipe = Pipeline::new(device);
        let glyphs = GlyphCache::new(fonts, Scale::default());
        let atlases = DeviceAtlases::new(device, &pipe, &glyphs);
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
            atlases,
            blank,
            target: None,
            size: PixelSize::new(0, 0),
            rows: Vec::new(),
            epoch: 0,
            tracker: ChangeTracker::default(),
            scene: RowScene::default(),
            instances: None,
            images: HashMap::new(),
            damage: Vec::new(),
        }
    }

    pub(super) fn metrics(&mut self, scale: Scale) -> CellMetrics {
        self.glyphs.set_scale(scale);
        self.glyphs.metrics()
    }
}
