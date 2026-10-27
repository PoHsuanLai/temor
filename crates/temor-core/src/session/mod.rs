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
pub mod found;
pub mod frame;
pub mod list;
mod marks;
mod shell_state;

pub use annotations::{AnnotateIn, AnnotateOut, Annotations, annotate_step};
pub use cards::{CardIn, CardOut, CardSize, Cards, Life, Placement, Surface, SurfaceId, card_step};
pub use checkpoint::{Checkpoint, CheckpointFault, CommandRecord};

/// The most bytes a pane's kept screen may have (4 MiB): one a terminal takes, or one written
/// for the next launch.
pub const SNAPSHOT_LIMIT: usize = 4 * 1024 * 1024;
pub use marks::{Anchor, BlockAnchors, KeptBlock, MarkIn, MarkKind, MarkLog, MarkOut, mark_step};
pub use shell_state::{
    EnvEntry, Replay, ShellDelta, ShellIn, ShellOut, ShellState, holds_secret, is_secret,
    looks_secret, never_carried, shell_step,
};
