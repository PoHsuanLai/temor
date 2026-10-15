//! Device copies of the glyph atlases and the kitty images, kept in step with memory.

use std::collections::HashMap;

use temor_vt::{Generation, ImageId};

use super::pipeline::{Pipeline, texture};
use crate::font::{Atlas, AtlasFormat, GlyphCache};
use crate::frame::ImagePlacement;

/// The two atlases on the device and the bind group that reads them.
#[derive(Debug)]
pub(super) struct DeviceAtlases {
    alpha: wgpu::Texture,
    color: wgpu::Texture,
    generations: (u64, u64),
    pub bind: wgpu::BindGroup,
}

fn atlas_texture(device: &wgpu::Device, atlas: &Atlas) -> wgpu::Texture {
    let format = match atlas.format {
        AtlasFormat::Alpha => wgpu::TextureFormat::R8Unorm,
        AtlasFormat::Color => wgpu::TextureFormat::Rgba8Unorm,
    };
    texture(
        device,
        "temor atlas",
        (atlas.width, atlas.height),
        format,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    )
}

pub(super) fn bind(
    device: &wgpu::Device,
    pipe: &Pipeline,
    alpha: &wgpu::Texture,
    color: &wgpu::Texture,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("temor atlases"),
        layout: &pipe.layout0,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: pipe.uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(
                    &alpha.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(
                    &color.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&pipe.sampler),
            },
        ],
    })
}

/// Copies `rows` texel rows of `atlas` starting at `(x, y)`, `w` wide.
fn write(
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    atlas: &Atlas,
    (x, y, w, h): (u32, u32, u32, u32),
) {
    let bpp = atlas.format.bytes_per_texel();
    let start = ((y * atlas.width + x) * bpp) as usize;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        &atlas.data[start..],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas.width * bpp),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
}

impl DeviceAtlases {
    pub fn new(device: &wgpu::Device, pipe: &Pipeline, glyphs: &GlyphCache) -> Self {
        let alpha = atlas_texture(device, &glyphs.alpha);
        let color = atlas_texture(device, &glyphs.color);
        let bind = bind(device, pipe, &alpha, &color);
        Self {
            alpha,
            color,
            // Never equal to an atlas's own generation: the first sync uploads everything.
            generations: (u64::MAX, u64::MAX),
            bind,
        }
    }

    /// Brings the device copies up to date; returns the bytes uploaded.
    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipe: &Pipeline,
        glyphs: &mut GlyphCache,
    ) -> u64 {
        let mut bytes = 0;
        let mut rebind = false;
        for (atlas, tex, generation) in [
            (&mut glyphs.alpha, &mut self.alpha, &mut self.generations.0),
            (&mut glyphs.color, &mut self.color, &mut self.generations.1),
        ] {
            let dirty = atlas.take_dirty();
            if *generation != atlas.generation {
                // Grown or cleared: a new texture, uploaded whole.
                *tex = atlas_texture(device, atlas);
                write(queue, tex, atlas, (0, 0, atlas.width, atlas.height));
                bytes += atlas.data.len() as u64;
                *generation = atlas.generation;
                rebind = true;
            } else {
                for s in dirty {
                    write(queue, tex, atlas, (s.x, s.y, s.w, s.h));
                    bytes += u64::from(s.w * s.h * atlas.format.bytes_per_texel());
                }
            }
        }
        if rebind {
            self.bind = bind(device, pipe, &self.alpha, &self.color);
        }
        bytes
    }
}

/// A kitty image on the device, keyed by id and replaced when its generation moves.
#[derive(Debug)]
pub(super) struct DeviceImage {
    generation: Generation,
    pub bind: wgpu::BindGroup,
}

/// Why a kitty image is not drawn on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageSkip {
    /// The image has no pixels.
    Empty,
    /// The image is wider or taller than the device's texture limit.
    TooLarge { width: u32, height: u32, limit: u32 },
}

/// Uploads `p`'s image unless the same generation is there already. An image the device cannot
/// hold is skipped, with the reason, rather than failing the frame.
pub(super) fn ensure_image(
    images: &mut HashMap<ImageId, DeviceImage>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipe: &Pipeline,
    p: &ImagePlacement,
    limit: u32,
) -> Result<(), ImageSkip> {
    let img = &p.image;
    if img.width == 0 || img.height == 0 {
        return Err(ImageSkip::Empty);
    }
    if img.width > limit || img.height > limit {
        return Err(ImageSkip::TooLarge {
            width: img.width,
            height: img.height,
            limit,
        });
    }
    if images
        .get(&img.id)
        .is_some_and(|d| d.generation == img.generation)
    {
        return Ok(());
    }
    let tex = texture(
        device,
        "kitty image",
        (img.width, img.height),
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &img.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(img.width * 4),
            rows_per_image: Some(img.height),
        },
        wgpu::Extent3d {
            width: img.width.max(1),
            height: img.height.max(1),
            depth_or_array_layers: 1,
        },
    );
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("kitty image"),
        layout: &pipe.layout1,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&tex.create_view(&Default::default())),
        }],
    });
    images.insert(
        img.id,
        DeviceImage {
            generation: img.generation,
            bind,
        },
    );
    Ok(())
}
