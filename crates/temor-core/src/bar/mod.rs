//! The ⌘T command bar (§7, D11): candidate rows, the fixed ranking and the bar's machine.

mod model;
mod rank;
mod step;
#[cfg(test)]
mod tests;

pub use model::{
    BarAction, BarRow, Candidate, Frecency, LayoutName, PathResolver, PluginRow, RowGroup, RowKey,
    Target,
};
pub use rank::rank;
pub use step::{BarContext, BarIn, BarKey, BarOut, BarState, OpenBar, bar_step};
