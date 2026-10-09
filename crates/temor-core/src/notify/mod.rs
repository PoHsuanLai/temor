//! Notices the person may get while the pane is unfocused (PLAN §8.1): a long command finishing,
//! a program's own notification (OSC 9, 99, 777) and the bell. Pure: the caller says whether
//! the pane has focus, and the edge decides what to show.

mod model;
mod policy;
#[cfg(test)]
mod tests;

pub use model::{Focus, Notice, NotifyAfter};
pub use policy::{Limiter, bell_notice, finished_notice, program_notice};
