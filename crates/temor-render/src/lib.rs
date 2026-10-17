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
//! # Consumers
//!
//! This crate draws with vello_cpu, which must be quire's fork (the colour matrix and filter
//! layers). The fork is wired by a `[patch.crates-io]` block, and `[patch]` applies only at a
//! workspace root, so a consumer that builds `temor-render` must copy quire's block into its own
//! root `Cargo.toml`. `scripts/fork-patches.sh print` in quire prints it.
//!
//! Fonts come from fontique ([`FontSet`]): the system `monospace` family by default, or a named
//! one, at a logical size. swash shapes and rasterises into glyph atlases; COLRv1 emoji are
//! painted through vello_cpu (skrifa's colour painter) into the colour atlas; box drawing, block
//! elements and Powerline arrows are drawn procedurally. [`CellMetrics`] and [`GridFit`] size the
//! grid in whole device pixels and map pixels to cells. [`RowLayout`] shows the drawn grid in
//! slices with room between rows (a picture under a command's output) without redrawing it.
//!
//! # Example
//!
//! Sizing a pane's grid in whole device pixels from a cell's metrics. Here the cell is 16 by 32
//! device pixels at scale 2, with 8 device pixels of padding on each side:
//!
//! ```
//! use temor_render::{CellMetrics, Padding, PixelSize, Scale, fit_grid};
//! use temor_vt::{CellPx, Px};
//!
//! let metrics = CellMetrics {
//!     cell: CellPx::new(16, 32),
//!     baseline: Px(24),
//!     underline: Px(28),
//!     strikeout: Px(16),
//!     thickness: Px(2),
//! };
//! let fit = fit_grid(PixelSize::new(1280, 800), &metrics, Padding::logical(4.0, Scale::new(2.0)));
//! assert_eq!(fit.grid.cols.0, 79);
//! assert_eq!(fit.grid.rows.0, 24);
//! ```

mod bands;
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

pub use bands::{Alpha, Cut, RowBand, RowSpan, RowWeight, STRIPE_LOGICAL, Tint};
pub use cpu::{CpuFrame, CpuRenderer};
pub use error::RenderError;
pub use font::{FontConfig, FontFamily, FontSet, FontSource};
pub use frame::PaneFrame;
#[cfg(feature = "ghostty")]
pub use frame::{CapturedFrame, FrameCapture};
#[cfg(feature = "gpu")]
pub use gpu::{GpuFrame, GpuRenderer};
pub use metrics::{
    CellHit, CellMetrics, CellPos, CellSide, FontSize, Gap, GridFit, GridHit, HitRegion, Insert,
    LayoutParams, Padding, Pin, PixelSize, RowLayout, Scale, Screen, Slice, fit_grid,
};
pub use palette::Palette;
pub use renderer::{
    BlinkPhase, DamageRect, PaneFocus, PaneView, RenderStats, Renderer, SelectionOverlay,
    SelectionShape,
};
