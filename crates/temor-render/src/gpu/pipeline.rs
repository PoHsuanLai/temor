//! The instanced quad pipeline (ported from the render spike): solid quads, alpha-atlas glyphs,
//! colour-atlas glyphs, and images sampled from their own bind group.

use bytemuck::{Pod, Zeroable};

/// One quad instance.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(super) struct Inst {
    /// x, y, width, height in target pixels.
    pub rect: [f32; 4],
    /// Atlas texels (glyphs) or normalised coordinates (images): x0, y0, x1, y1.
    pub uv: [f32; 4],
    /// Premultiplied colour.
    pub color: [f32; 4],
    /// 0 solid, 1 alpha glyph, 2 colour glyph, 3 image.
    pub kind: u32,
    pub _pad: [u32; 3],
}

pub(super) const SOLID: u32 = 0;
pub(super) const ALPHA: u32 = 1;
pub(super) const COLOR: u32 = 2;
pub(super) const IMAGE: u32 = 3;

const SHADER: &str = r#"
struct Inst { @location(0) rect: vec4<f32>, @location(1) uv: vec4<f32>, @location(2) color: vec4<f32>, @location(3) kind: u32 };
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32>, @location(2) @interpolate(flat) kind: u32 };
struct View { size: vec2<f32>, _pad: vec2<f32> };
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var alpha_atlas: texture_2d<f32>;
@group(0) @binding(2) var color_atlas: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
@group(1) @binding(0) var image: texture_2d<f32>;

@vertex fn vs(@builtin(vertex_index) vi: u32, i: Inst) -> Out {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = i.rect.xy + corner * i.rect.zw;
    var o: Out;
    o.pos = vec4<f32>(p.x / view.size.x * 2.0 - 1.0, 1.0 - p.y / view.size.y * 2.0, 0.0, 1.0);
    o.uv = mix(i.uv.xy, i.uv.zw, corner);
    o.color = i.color;
    o.kind = i.kind;
    return o;
}

@fragment fn fs(o: Out) -> @location(0) vec4<f32> {
    switch o.kind {
        case 0u: { return o.color; }
        case 1u: {
            let a = textureLoad(alpha_atlas, vec2<i32>(floor(o.uv)), 0).r;
            return o.color * a;
        }
        case 2u: { return textureLoad(color_atlas, vec2<i32>(floor(o.uv)), 0); }
        default: { return textureSampleLevel(image, samp, o.uv, 0.0); }
    }
}
"#;

/// The compiled pipeline and its layouts.
#[derive(Debug)]
pub(super) struct Pipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub layout0: wgpu::BindGroupLayout,
    pub layout1: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
    pub uniform: wgpu::Buffer,
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl Pipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("temor cells"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("temor cells"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("temor image"),
            entries: &[texture_entry(0)],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("temor cells"),
            bind_group_layouts: &[Some(&layout0), Some(&layout1)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("temor cells"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Inst>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Uint32],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("temor view"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout0,
            layout1,
            sampler,
            uniform,
        }
    }
}

/// A 2D texture the pass samples or renders into.
pub(super) fn texture(
    device: &wgpu::Device,
    label: &str,
    (w, h): (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}
