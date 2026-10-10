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
mod marks;
mod model;
mod offer;
mod open;
mod pointer;
mod read;
mod viewer;

pub use clip::{PASTE_LIMIT, PendingPaste};
pub use clipboard::{Clipboard, ClipboardFailed};
pub use find::{FindStep, QUERY_DEBOUNCE};
pub use lifecycle::{BELL_MARK, SNAPSHOT_LIMIT, sanitise_title};
pub use marks::{BlockMark, PromptStep, ToolsHeight, desktop_note};
pub use model::{Drag, FedParts, Notice, PADDING_LOGICAL, Session, SessionKit, SessionParts};
pub use pointer::WheelDirection;
pub use viewer::ViewAsk;
