//! Cell metrics, grid fitting, pixel↔cell mapping and room between rows.

mod fit;
mod layout;
mod layout_types;
mod model;
#[cfg(test)]
mod tests;

pub use fit::fit_grid;
pub use layout::RowLayout;
pub use layout_types::{Gap, GridHit, Insert, LayoutParams, Pin, Screen, Slice};
pub use model::{
    CellHit, CellMetrics, CellPos, CellSide, FontSize, GridFit, HitRegion, LineHeight, Padding,
    PixelSize, Scale,
};
