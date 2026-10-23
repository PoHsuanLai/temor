//! A pane's session as plain data and pure steps (PLAN: session as data). The edges feed these
//! functions facts and carry out what they return; nothing here reads a terminal, a clock or a
//! file.

mod annotations;
#[cfg(test)]
mod annotations_tests;

mod cards;
#[cfg(test)]
mod cards_tests;
mod checkpoint;
pub mod frame;
mod marks;
mod shell_state;

pub use annotations::{AnnotateIn, AnnotateOut, Annotations, annotate_step};
pub use cards::{CardIn, CardOut, CardSize, Cards, Life, Placement, Surface, SurfaceId, card_step};
pub use checkpoint::{Checkpoint, CheckpointFault, CommandRecord};
pub use marks::{
    Anchor, BlockAnchors, HeaderFacts, MarkIn, MarkKind, MarkLog, MarkOut, PluginFact, mark_step,
};
pub use shell_state::{
    EnvEntry, Replay, ShellDelta, ShellIn, ShellOut, ShellState, is_secret, shell_step,
};
