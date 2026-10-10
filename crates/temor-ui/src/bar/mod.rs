//! The ⌘T bar in the window (PLAN §7, D11): quire's palette machine over the rows temor-core
//! ranks, the providers wired at the edge, and the seams other parts of the app fill in (Spaces,
//! hosts, the model). Pure parts here; the hub carries the picks out (`hub::bar`).

mod adapter;
#[cfg(test)]
mod adapter_tests;
mod resolver;
mod seams;
mod shortcuts;
mod sources;
#[cfg(test)]
mod sources_tests;
mod viewer_find;
#[cfg(test)]
mod viewer_find_tests;

pub use adapter::BarModel;
pub use resolver::PathLookup;
pub use seams::{BarSeams, ModelRows, NoModel, NoSpaces, SpaceRows};
pub use shortcuts::action_of;
pub use sources::{closed_candidate, directory_uses, recent_commands};
pub use viewer_find::{
    Escape, Finding, PLACEHOLDER as FIND_PLACEHOLDER, Reply, entry_row, escape, lift, reply,
};
