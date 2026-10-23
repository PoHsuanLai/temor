//! A pane's session as plain data and pure steps (PLAN: session as data). The edges feed these
//! functions facts and carry out what they return; nothing here reads a terminal, a clock or a
//! file.

mod annotations;
#[cfg(test)]
mod annotations_tests;

pub mod frame;
mod marks;

pub use annotations::{AnnotateIn, AnnotateOut, Annotations, annotate_step};
pub use marks::{Anchor, BlockAnchors, MarkIn, MarkKind, MarkLog, MarkOut, mark_step};
