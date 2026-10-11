//! One pane as the window drives it: the engine's handle, the painter, the selection, the find
//! bar and the paste confirmation. The window sends it events and reads back what to show; it
//! never touches the terminal or the PTY itself. Dioxus is not used here.

mod blocks;
mod clip;
mod clipboard;
mod dropped;
mod edit_text;
mod find;
mod keys;
mod lifecycle;
mod links;
mod marks;
mod model;
mod offer;
mod open;
mod osc52;
mod pointer;
mod read;
mod running;
mod viewer;

pub use clip::{PASTE_LIMIT, PendingPaste};
pub use clipboard::{Clipboard, ClipboardFailed};
pub use find::{FindStep, QUERY_DEBOUNCE};
pub use lifecycle::{BELL_MARK, SNAPSHOT_LIMIT, sanitise_title};
pub use links::{LinkHover, LinkTipSpot};
pub use marks::{BlockMark, PromptStep, ToolsHeight, desktop_note};
pub use model::{Drag, FedParts, Notice, PADDING_LOGICAL, Session, SessionKit, SessionParts};
pub use osc52::{ClipboardAsk, ClipboardUse};
pub use pointer::WheelDirection;
pub use running::Running;
pub use viewer::ViewAsk;
