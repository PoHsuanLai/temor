//! The GPU renderer on a real device (skipped where wgpu finds no adapter): a frame whose glyphs
//! clear the atlas of the last frame's matches one drawn with room, an image wider than the device allows is skipped
//! without failing the frame, and bands draw as the CPU draws them.

use std::sync::Arc;

use temor_vt::{CellStyle, Col, GridSize, KittyLayer, Px, Rgb, Row, Selected};

use super::GpuRenderer;
use crate::bands::{Alpha, Cut, RowBand, RowSpan, RowWeight, Tint};
use crate::cpu::CpuRenderer;
use crate::font::testing::fonts;
use crate::frame::{CellDraw, CellText, ImagePixels, ImagePlacement, PaneFrame, TermColors};
use crate::metrics::{Padding, PixelSize, Scale};
use crate::palette::Palette;
use crate::renderer::{BlinkPhase, PaneFocus, PaneView, Renderer};

const COLS: u16 = 40;
const ROWS: u16 = 8;
const PAD: u32 = 4;
/// Distinct glyphs per frame in the atlas test: one frame's fit the small atlas, two don't.
const SHARE: usize = 60;

/// A device with no window: Vulkan or whatever wgpu finds headless; `None` skips the test.
fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

/// The texture's pixels, tightly packed RGBA.
fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture) -> Vec<u8> {
    let (w, h) = (tex.width(), tex.height());
    let bpr = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("read back"),
        size: u64::from(bpr * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * bpr) as usize;
        out.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    out
}

/// Box-drawing characters, one distinct mask each.
fn frame() -> PaneFrame {
    glyphs(0, 160)
}

/// Cells cycling through `count` box-drawing characters from U+2500 + `first`.
fn glyphs(first: u32, count: usize) -> PaneFrame {
    let mut f = PaneFrame::new();
    f.resize(GridSize::new(COLS, ROWS));
    f.set_colors(TermColors {
        foreground: Rgb::new(220, 220, 220),
        background: Rgb::new(0, 0, 0),
        cursor: None,
    });
    for r in 0..ROWS {
        let cells = (0..COLS)
            .map(|c| {
                let i = usize::from(r) * usize::from(COLS) + usize::from(c);
                let text = char::from_u32(0x2500 + first + (i % count) as u32)
                    .map_or_else(|| "?".to_owned(), |ch| ch.to_string());
                CellDraw {
                    col: Col(c),
                    span: 1,
                    text: CellText::new(&text),
                    fg: None,
                    bg: None,
                    style: CellStyle::default(),
                    selected: Selected::No,
                }
            })
            .collect();
        f.set_row(Row(r), cells);
    }
    f
}

fn size_for(renderer: &mut impl Renderer) -> PixelSize {
    let m = renderer.cell_metrics(Scale::new(1.0));
    PixelSize::new(
        u32::from(COLS) * m.width() + PAD * 2,
        u32::from(ROWS) * m.height() + PAD * 2,
    )
}

fn view<'a>(size: PixelSize, palette: &'a Palette, bands: &'a [RowBand]) -> PaneView<'a> {
    PaneView {
        size,
        scale: Scale::new(1.0),
        padding: Padding::uniform(Px(PAD)),
        palette,
        focus: PaneFocus::Focused,
        blink: BlinkPhase::On,
        selection: None,
        bands,
        lower: 0,
    }
}

/// One GPU frame of `f`: its pixels and stats.
fn gpu_draw(
    (device, queue): &(wgpu::Device, wgpu::Queue),
    gpu: &mut GpuRenderer,
    mut f: PaneFrame,
    bands: &[RowBand],
) -> (Vec<u8>, crate::renderer::RenderStats) {
    let size = size_for(gpu);
    let palette = Palette::default();
    let out = gpu
        .render(&mut f, &view(size, &palette, bands))
        .expect("gpu render");
    let stats = out.stats;
    queue.submit([out.commands]);
    let tex = out.texture;
    (read_back(device, queue, tex), stats)
}

/// The share of pixels that differ by more than `tolerance` in any channel.
fn differ(a: &[u8], b: &[u8], tolerance: u8) -> f64 {
    let off = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .filter(|(x, y)| {
            x.iter()
                .zip(y.iter())
                .any(|(p, q)| p.abs_diff(*q) > tolerance)
        })
        .count();
    off as f64 / (a.len() / 4) as f64
}

/// A frame draws with all its glyphs in the atlas at once, so a frame that needs room clears
/// what the last frame left and builds again; it must then match one drawn with room.
#[test]
fn a_gpu_frame_that_clears_the_last_frames_glyphs_matches_one_drawn_with_room() {
    let Some(dev) = device() else {
        eprintln!("no wgpu adapter: skipped");
        return;
    };
    let mut roomy = GpuRenderer::new(&dev.0, &dev.1, fonts(60.0));
    let mut tight = GpuRenderer::with_atlas_caps(&dev.0, &dev.1, fonts(60.0), 256, 256);
    let (_, first) = gpu_draw(&dev, &mut tight, glyphs(0, SHARE), &[]);
    assert_eq!(
        first.atlas_resets, 0,
        "one frame's glyphs fit the small atlas"
    );
    let (tight_px, tight_stats) = gpu_draw(&dev, &mut tight, glyphs(60, SHARE), &[]);
    let (roomy_px, roomy_stats) = gpu_draw(&dev, &mut roomy, glyphs(60, SHARE), &[]);
    assert_eq!(roomy_stats.atlas_resets, 0);
    assert!(
        tight_stats.atlas_resets > 0,
        "two frames' glyphs must not fit together for this test to mean anything"
    );
    assert!(
        roomy_px == tight_px,
        "a glyph was read from a cleared atlas"
    );
}

/// A frame whose own glyphs outgrow the atlas is drawn in groups of rows, each in a pass of its
/// own with the atlases as they were for it; the picture must match one drawn with room, and the
/// next frame (which fits) must too.
#[test]
fn a_gpu_frame_whose_own_glyphs_outgrow_the_atlas_is_drawn_in_groups() {
    let Some(dev) = device() else {
        eprintln!("no wgpu adapter: skipped");
        return;
    };
    let mut roomy = GpuRenderer::new(&dev.0, &dev.1, fonts(60.0));
    let mut tight = GpuRenderer::with_atlas_caps(&dev.0, &dev.1, fonts(60.0), 256, 256);
    // (name, first glyph, how many distinct glyphs the frame holds)
    let frames = [
        ("160 glyphs, more than the atlas holds", 0, 160),
        ("a frame that fits again afterwards", 0, SHARE),
    ];
    for (name, first, count) in frames {
        let (roomy_px, roomy_stats) = gpu_draw(&dev, &mut roomy, glyphs(first, count), &[]);
        let (tight_px, tight_stats) = gpu_draw(&dev, &mut tight, glyphs(first, count), &[]);
        assert_eq!(roomy_stats.atlas_resets, 0, "{name}");
        assert!(roomy_px == tight_px, "{name}: the pictures differ");
        if count > SHARE {
            assert!(
                tight_stats.atlas_resets > 0,
                "{name}: the glyphs must not fit for this test to mean anything"
            );
        }
    }
}

#[test]
fn an_image_wider_than_the_device_allows_is_skipped_and_the_frame_still_draws() {
    let Some(dev) = device() else {
        eprintln!("no wgpu adapter: skipped");
        return;
    };
    let limit = dev.0.limits().max_texture_dimension_2d;
    let width = limit + 1;
    let mut f = frame();
    let placement = |id: u32, width: u32| ImagePlacement {
        layer: KittyLayer::AboveText,
        image: Arc::new(ImagePixels {
            id: temor_vt::ImageId(id),
            generation: temor_vt::Generation(1),
            width,
            height: 1,
            rgba: vec![255; (width * 4) as usize],
        }),
        col: 0,
        row: 0,
        offset: (0, 0),
        width: Px(40),
        height: Px(4),
        source: [0, 0, width, 1],
    };
    f.images = vec![placement(1, width), placement(2, 8)];
    let mut gpu = GpuRenderer::new(&dev.0, &dev.1, fonts(14.0));
    let (px, stats) = gpu_draw(&dev, &mut gpu, f, &[]);
    assert_eq!(
        stats.images_skipped, 1,
        "only the image past {limit} px is skipped"
    );
    assert!(!px.is_empty());
}

#[test]
fn bands_draw_on_the_gpu_as_on_the_cpu() {
    let Some(dev) = device() else {
        eprintln!("no wgpu adapter: skipped");
        return;
    };
    let tint = |r, g, b, a| {
        Some(Tint {
            color: Rgb::new(r, g, b),
            alpha: Alpha(a),
        })
    };
    let bands = [
        RowBand {
            rows: RowSpan {
                first: Row(1),
                last: Row(2),
            },
            top: Cut::Edge,
            bottom: Cut::Middle,
            fill: tint(200, 0, 0, 128),
            stripe: tint(0, 255, 0, 255),
            rule: tint(0, 0, 255, 255),
            weight: RowWeight::Normal,
        },
        RowBand {
            rows: RowSpan {
                first: Row(5),
                last: Row(5),
            },
            top: Cut::Middle,
            bottom: Cut::Edge,
            fill: tint(0, 0, 200, 200),
            stripe: None,
            rule: None,
            weight: RowWeight::Normal,
        },
    ];
    let mut cpu = CpuRenderer::new(fonts(14.0));
    let size = size_for(&mut cpu);
    let palette = Palette::default();
    let mut f = frame();
    let cpu_px = cpu
        .render(&mut f, &view(size, &palette, &bands))
        .expect("cpu render")
        .pixels
        .to_vec();
    let mut gpu = GpuRenderer::new(&dev.0, &dev.1, fonts(14.0));
    let (gpu_px, _) = gpu_draw(&dev, &mut gpu, frame(), &bands);
    assert_eq!(cpu_px.len(), gpu_px.len());
    let share = differ(&cpu_px, &gpu_px, 24);
    assert!(share < 0.02, "{:.2}% of pixels differ", share * 100.0);
}
