//! The ⌘T command bar (§7, D11): candidate rows, the fixed ranking and the bar's machine.

mod frecency;
mod model;
#[cfg(test)]
mod provider_tests;
mod providers;
mod rank;
mod step;
#[cfg(test)]
mod tests;

pub use frecency::{Recency, UnixSecs, Visits};
pub use model::{
    BarAction, BarRow, Candidate, Frecency, LayoutName, PathResolver, PluginRow, RowGroup, RowKey,
    Target,
};
pub use providers::{DirectoryUse, Sources, SpaceEntry, candidates, space_entries};
pub use rank::rank;
pub use step::{BarContext, BarIn, BarOut, BarState, OpenBar, bar_step};
