//! temor's terminal renderer (PLAN §5).
//!
//! A frame flows in three steps:
//!
//! 1. **Capture** ([`FrameCapture`], feature `ghostty`). Under the pane's terminal lock,
//!    [`FrameCapture::begin`] starts libghostty's render update and reads the kitty placements
//!    (they need `&Terminal`). Outside the lock, [`CapturedFrame::finish`] walks only the dirty
//!    rows into an owned [`PaneFrame`].
//! 2. **Render** through the [`Renderer`] trait, for a pane of a given device-pixel size and
//!    scale ([`PaneView`]). [`CpuRenderer`] (the default) composes only the rows that changed
//!    into a premultiplied RGBA8 buffer and reports [`DamageRect`]s so the caller uploads only
//!    those. `GpuRenderer` (feature `gpu`) records one instanced pass into a texture it owns.
//! 3. **Show**: the UI uploads the pixels (or registers the texture) in a quire `TextureLayer`.
//!    Nothing here names Dioxus or ds-blitz.
//!
//! Fonts come from fontique ([`FontSet`]): the system `monospace` family by default, or a named
//! one, at a logical size. swash shapes and rasterises into glyph atlases; COLRv1 emoji are
//! painted through vello_cpu (skrifa's colour painter) into the colour atlas; box drawing, block
//! elements and Powerline arrows are drawn procedurally. [`CellMetrics`] and [`GridFit`] size the
//! grid in whole device pixels and map pixels to cells.

mod error;
mod font;
mod frame;
mod metrics;
mod palette;
mod platform;
mod renderer;
mod scene;
mod track;

mod cpu;
#[cfg(feature = "gpu")]
mod gpu;

pub use cpu::{CpuFrame, CpuRenderer};
pub use error::RenderError;
pub use font::{FontConfig, FontFamily, FontSet, FontSource};
pub use frame::PaneFrame;
#[cfg(feature = "ghostty")]
pub use frame::{CapturedFrame, FrameCapture};
#[cfg(feature = "gpu")]
pub use gpu::{GpuFrame, GpuRenderer};
pub use metrics::{
    CellHit, CellMetrics, CellPos, CellSide, FontSize, GridFit, HitRegion, Padding, PixelSize,
    Scale, fit_grid,
};
pub use palette::Palette;
pub use renderer::{
    DamageRect, PaneFocus, PaneView, RenderStats, Renderer, SelectionOverlay, SelectionShape,
};
