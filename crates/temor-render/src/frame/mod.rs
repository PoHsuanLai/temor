//! The frame model and, with feature `ghostty`, capturing it from a terminal.

#[cfg(feature = "ghostty")]
mod capture;
mod diacritics;
#[cfg(feature = "ghostty")]
mod images;
mod model;
mod placeholder;
mod update;
mod virtual_images;

#[cfg(feature = "ghostty")]
pub use capture::{CapturedFrame, FrameCapture};
#[cfg(any(test, feature = "testing"))]
pub(crate) use model::CellText;
#[cfg(all(test, feature = "gpu"))]
pub(crate) use model::ImagePixels;
pub use model::PaneFrame;
pub(crate) use model::{CellDraw, ImagePlacement, TermColors};
