//! The GPU renderer (feature `gpu`, PLAN §5): one instanced WGSL pass over the glyph atlases,
//! with kitty images by z-layer, into a texture this renderer owns on the window's device.
//!
//! **It never submits.** [`GpuRenderer::render`] uploads with `queue.write_*` and hands back a
//! recorded [`wgpu::CommandBuffer`]; the caller submits it on the window's render thread. A
//! `queue.submit` from a pane thread stalls up to one refresh against the window's present
//! (spike P0(b)), so do not submit `GpuFrame::commands` from a pane thread: wait for quire's
//! render hook (v0.2.32), which encodes into the layer's texture before the window's own submit.
//! Until then the CPU renderer is the default.

mod device;
mod frame;
mod pipeline;

use std::collections::HashMap;

use temor_vt::ImageId;

use crate::font::{FontSet, GlyphCache};
use crate::metrics::{CellMetrics, GridFit, PixelSize, Scale};
use crate::renderer::{DamageRect, RenderStats};
use crate::scene::RowScene;
use crate::track::ChangeTracker;
use device::{DeviceAtlases, DeviceImage};
use pipeline::{Inst, Pipeline};

/// Renders panes on a wgpu device into its own `Rgba8Unorm` (premultiplied) texture.
#[derive(Debug)]
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipe: Pipeline,
    glyphs: GlyphCache,
    atlases: DeviceAtlases,
    blank: wgpu::BindGroup,
    target: Option<wgpu::Texture>,
    size: PixelSize,
    /// Per grid row: background and foreground instances, rebuilt only for composed rows.
    rows: Vec<(Vec<Inst>, Vec<Inst>)>,
    /// The glyph-cache epoch the cached rows were built in.
    epoch: u64,
    tracker: ChangeTracker,
    scene: RowScene,
    instances: Option<wgpu::Buffer>,
    images: HashMap<ImageId, DeviceImage>,
    damage: Vec<DamageRect>,
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

    fn metrics(&mut self, scale: Scale) -> CellMetrics {
        self.glyphs.set_scale(scale);
        self.glyphs.metrics()
    }
}
