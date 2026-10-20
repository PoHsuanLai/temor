//! The GPU renderer on a real device (skipped where wgpu finds no adapter): a frame whose glyphs
//! clear the atlas of the last frame's matches one drawn with room, an image wider than the device allows is skipped
//! without failing the frame, and bands draw as the CPU draws them.

use std::sync::Arc;

use temor_vt::{KittyLayer, Px, Rgb, Row};

use super::GpuRenderer;
use crate::bands::{Alpha, Cut, RowBand, RowSpan, RowWeight, Tint};
use crate::cpu::CpuRenderer;
use crate::frame::{ImagePixels, ImagePlacement, PaneFrame};
use crate::palette::Palette;
use crate::renderer::Renderer;
use crate::testing::{device, fonts, glyph_frame, read_back, size_for, view};

/// Distinct glyphs per frame in the atlas test: one frame's fit the small atlas, two don't.
const SHARE: usize = 60;

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
    let (_, first) = gpu_draw(&dev, &mut tight, glyph_frame(0, SHARE), &[]);
    assert_eq!(
        first.atlas_resets, 0,
        "one frame's glyphs fit the small atlas"
    );
    let (tight_px, tight_stats) = gpu_draw(&dev, &mut tight, glyph_frame(60, SHARE), &[]);
    let (roomy_px, roomy_stats) = gpu_draw(&dev, &mut roomy, glyph_frame(60, SHARE), &[]);
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
        let (roomy_px, roomy_stats) = gpu_draw(&dev, &mut roomy, glyph_frame(first, count), &[]);
        let (tight_px, tight_stats) = gpu_draw(&dev, &mut tight, glyph_frame(first, count), &[]);
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
    let mut f = glyph_frame(0, 160);
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
    let mut f = glyph_frame(0, 160);
    let cpu_px = cpu
        .render(&mut f, &view(size, &palette, &bands))
        .expect("cpu render")
        .pixels
        .to_vec();
    let mut gpu = GpuRenderer::new(&dev.0, &dev.1, fonts(14.0));
    let (gpu_px, _) = gpu_draw(&dev, &mut gpu, glyph_frame(0, 160), &bands);
    assert_eq!(cpu_px.len(), gpu_px.len());
    let share = differ(&cpu_px, &gpu_px, 24);
    assert!(share < 0.02, "{:.2}% of pixels differ", share * 100.0);
}
