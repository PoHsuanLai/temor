//! The quick terminal (§7): one hotkey opens a small throwaway terminal over the current window,
//! and "Move to Space" promotes it into a real tab.

mod adopt;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use adopt::adopt;
pub use model::{QuickIn, QuickOut, QuickState, QuickTerm};
pub use step::quick_step;
