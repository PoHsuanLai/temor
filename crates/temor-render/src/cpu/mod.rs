//! The CPU renderer (the default, PLAN §5): dirty rows composed into a premultiplied RGBA8
//! buffer the size of the pane, with the changed bands reported for partial uploads.

mod compose;
mod pixels;
mod renderer;

#[cfg(test)]
mod tests;

pub use renderer::{CpuFrame, CpuRenderer};
