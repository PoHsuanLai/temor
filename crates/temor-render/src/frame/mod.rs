//! The frame model and, with feature `ghostty`, capturing it from a terminal.

#[cfg(feature = "ghostty")]
mod capture;
#[cfg(feature = "ghostty")]
mod images;
mod model;
mod update;

#[cfg(feature = "ghostty")]
pub use capture::{CapturedFrame, FrameCapture};
#[cfg(test)]
pub(crate) use model::CellText;
pub use model::PaneFrame;
pub(crate) use model::{CellDraw, ImagePlacement, TermColors};
