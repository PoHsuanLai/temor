//! One pane as the window drives it: the engine's handle, the painter, the selection, the find
//! bar and the paste confirmation. The window sends it events and reads back what to show; it
//! never touches the terminal or the PTY itself. Dioxus is not used here.

mod blink;
mod block_list;
mod blocks;
mod clip;
mod clipboard;
mod composer;
mod dropped;
mod edit_text;
mod find;
mod keys;
mod lifecycle;
mod links;
mod list_hit;
mod look;
mod marks;
mod model;
mod offer;
mod open;
mod osc52;
mod pointer;
mod read;
mod room;
mod running;
mod viewer;

pub use blink::{BLINK_MS, blink_phase};
pub use block_list::{CARD_MARGIN, LiveRows};
pub use clip::{PASTE_LIMIT, PendingPaste};
pub use clipboard::{Clipboard, ClipboardFailed};
pub use composer::HistoryStep;
pub use find::{FindStep, QUERY_DEBOUNCE};
pub use lifecycle::{BELL_MARK, SNAPSHOT_LIMIT, sanitise_title};
pub use links::{LinkHover, LinkTipSpot};
pub use list_hit::{ListMap, ListSpan, Toward};
pub use marks::desktop_note;
pub use model::{
    Drag, FedParts, INSET, Notice, PADDING_LOGICAL, Session, SessionKit, SessionParts,
};
pub use osc52::{ClipboardAsk, ClipboardUse};
pub use pointer::WheelDirection;
pub use room::{LENS_MOST_ROWS, Placed, Relaid};
pub use running::Running;
pub use temor_core::session::HeaderFacts;
pub use viewer::ViewAsk;
