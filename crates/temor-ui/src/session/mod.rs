//! One pane as the window drives it: the engine's handle, the painter, the selection, the find
//! bar and the paste confirmation. The window sends it events and reads back what to show; it
//! never touches the terminal or the PTY itself. Dioxus is not used here.

mod clip;
mod clipboard;
mod find;
mod keys;
mod lifecycle;
mod model;
mod pointer;
mod read;

pub use clip::{PASTE_LIMIT, PendingPaste};
pub use clipboard::{Clipboard, ClipboardFailed};
pub use find::{FindStep, QUERY_DEBOUNCE};
pub use lifecycle::sanitise_title;
pub use model::{Drag, Notice, PADDING_LOGICAL, Session, SessionParts};
pub use pointer::WheelDirection;
