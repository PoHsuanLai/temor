//! Shell marks and blocks in the session (PLAN §9 blocks). Each OSC 133 mark arrives numbered: its
//! `MarkBegin`, its own event, then its `Anchor` with the same number. The anchor's row feeds the
//! block machine, a finished block may notify, and the prompt jumps and the block
//! actions read the anchors back.

mod actions;
mod lead;
mod mark;
mod notify;
mod picks;
mod query;
pub use temor_client::pane::marks::Finished;
mod rows;
mod survey;
#[cfg(test)]
mod tests;

pub use notify::desktop_note;
