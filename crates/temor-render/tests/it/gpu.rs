//! The GPU renderer draws the same picture as the CPU one, when a wgpu adapter is available
//! (skipped otherwise). The test submits the recorded commands itself; real panes must not
//! (see `GpuRenderer`).

use temor_render::{GpuRenderer, Padding, PaneFocus, PaneView, Renderer};
use temor_vt::Px;

use crate::support::{PADDING, Pane, fixture, fonts};

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture) -> Vec<u8> {
    let (w, h) = (tex.width(), tex.height());
    let bpr = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bpr * h),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
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
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = buf.slice(..).get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let s = (y * bpr) as usize;
        out.extend_from_slice(&data[s..s + (w * 4) as usize]);
    }
    out
}

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
        selection: None,
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
