//! Cell metrics, grid fitting and pixel↔cell mapping.

mod fit;
mod model;
#[cfg(test)]
mod tests;

pub use fit::fit_grid;
pub use model::{
    CellHit, CellMetrics, CellPos, CellSide, FontSize, GridFit, HitRegion, Padding, PixelSize,
    Scale,
};
