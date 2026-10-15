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
mod groups;
mod pipeline;
mod record;
mod renderer;
#[cfg(test)]
mod tests;

pub use renderer::{GpuFrame, GpuRenderer};
