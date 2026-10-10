//! Row bands: tinted fills, stripes and rules laid over runs of viewport rows.

mod model;
#[cfg(test)]
mod tests;

pub use model::{Alpha, RowBand, RowSpan, STRIPE_LOGICAL, Tint};
pub(crate) use model::{RowBox, RowLook, look_of, push_quads};
