//! Fonts: discovery (fontique), shaping and rasterising (swash, vello_cpu for COLR), the glyph
//! cache, the GPU's glyph atlases, and the procedural box drawing.

#[cfg(any(feature = "gpu", test))]
#[cfg_attr(not(feature = "gpu"), allow(dead_code))]
mod atlas;
mod boxdraw;
mod cache;
mod catalog;
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
pub(crate) use atlas::Atlases;
#[cfg(feature = "gpu")]
pub(crate) use atlas::{Atlas, AtlasCopy, AtlasFormat, Overflow};
#[cfg(test)]
pub(crate) use cache::TEST_GLYPH;
pub(crate) use cache::{GlyphCache, GlyphKey, GlyphKind, RunGlyph};
pub use catalog::{FontCatalog, font_catalog, warm_font_catalog};
pub(crate) use config::{FaceStyle, Slant, Weight};
pub use config::{FontConfig, FontFamily, FontSource};
pub(crate) use decor::Decoration;
pub use set::FontSet;
