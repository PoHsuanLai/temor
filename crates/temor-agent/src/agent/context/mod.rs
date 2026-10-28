//! The context card (D21): before any model call, exactly what will be sent, as removable chips
//! with sizes; the list sent is kept with the request's log record. This module is the data and
//! its step; the model call itself is P8.

mod build;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use build::{ContextSources, OUTPUT_LINES, build_card};
pub use model::{Chip, ChipId, ChipKind, ContextCard, EntryPoint, Removal, SentChip, SentList};
pub use step::{ContextIn, ContextOut, ContextPhase, context_step};
