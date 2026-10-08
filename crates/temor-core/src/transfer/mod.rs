//! Transfers and the downloads shelf (§9, D12).

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Download, Origin, Transfer};
pub use step::{TransferIn, TransferOut, offer, transfer_step};
