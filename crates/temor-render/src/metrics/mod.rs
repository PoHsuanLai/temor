//! Cell metrics, grid fitting, pixel↔cell mapping and room between rows.

mod fit;
mod layout;
mod model;
#[cfg(test)]
mod tests;

pub use fit::fit_grid;
pub use layout::{Gap, GridHit, Insert, LayoutParams, Pin, RowLayout, Screen, Slice};
pub use model::{
    CellHit, CellMetrics, CellPos, CellSide, FontSize, GridFit, HitRegion, Padding, PixelSize,
    Scale,
};
