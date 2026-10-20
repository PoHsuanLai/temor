//! Fonts: discovery (fontique), shaping and rasterising (swash, vello_cpu for COLR), the glyph
//! atlases, and the procedural box drawing.

mod atlas;
mod boxdraw;
mod cache;
mod config;
mod decor;
mod face;
mod fallback;
mod measure;
mod place;
mod raster;
mod set;
#[cfg(test)]
mod tests;
#[cfg(feature = "gpu")]
pub(crate) use atlas::AtlasCopy;
#[cfg(any(feature = "gpu", test))]
pub(crate) use atlas::AtlasFormat;
pub(crate) use atlas::{Atlas, Slot};
pub(crate) use cache::{BUILD_ATTEMPTS, GlyphCache, GlyphKind, RunGlyph};
pub(crate) use config::{FaceStyle, Slant, Weight};
pub use config::{FontConfig, FontFamily, FontSource};
pub(crate) use decor::Decoration;
pub use set::FontSet;
