//! The GPU renderer draws the same picture as the CPU one, when a wgpu adapter is available
//! (skipped otherwise). The test submits the recorded commands itself; real panes must not
//! (see `GpuRenderer`).

use temor_render::testing::{device, read_back};
use temor_render::{GpuRenderer, Padding, PaneFocus, PaneView, Renderer};
use temor_vt::Px;

use crate::support::{PADDING, Pane, fixture, fonts};

#[test]
fn gpu_and_cpu_draw_the_same_frame() {
    let Some((device, queue)) = device() else {
        eprintln!("no wgpu adapter: skipped");
        return;
    };
    let mut pane = Pane::new(26, 4, 1.0, 14.0);
    pane.write(&fixture("mixed.vt"));
    let cpu = pane.render(PaneFocus::Focused);

    let mut gpu = GpuRenderer::new(&device, &queue, fonts(14.0));
    let view = PaneView {
        size: pane.size,
        scale: pane.scale,
        padding: Padding::uniform(Px(PADDING)),
        palette: &pane.palette,
        focus: PaneFocus::Focused,
        blink: temor_render::BlinkPhase::On,
        selection: None,
        bands: &[],
        lower: 0,
    };
    let out = gpu.render(&mut pane.frame, &view).expect("gpu render");
    assert_eq!(out.stats.rows_composed, 4);
    queue.submit([out.commands]);
    let pixels = read_back(&device, &queue, out.texture);
    let off = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(cpu.rgba.as_chunks::<4>().0.iter())
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 24))
        .count();
    let share = off as f64 / (pixels.len() / 4) as f64;
    assert!(share < 0.02, "{off} pixels differ ({:.2}%)", share * 100.0);
}
