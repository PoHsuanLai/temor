//! Shell marks and blocks in the session (PLAN §9 blocks). Each OSC 133 mark arrives numbered: its
//! `MarkBegin`, its own event, then its `Anchor` with the same number. The anchor's row feeds the
//! block machine, a finished block may notify, and the gutter, the prompt jumps and the block
//! actions read the anchors back.

mod actions;
mod anchors;
mod bands;
mod mark;
mod notify;
mod query;
mod rows;
#[cfg(test)]
mod tests;
mod tools_height;

pub use notify::desktop_note;
pub use query::{BlockMark, Gutter, PromptStep};
pub use tools_height::ToolsHeight;
