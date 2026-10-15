//! temor's terminal core. `Terminal` wraps libghostty-vt (feature `ghostty`, on by default):
//! bytes in, a typed [`VtEvent`] stream out through a caller-supplied [`EventSink`], and a
//! two-phase [`RenderView`] for the renderer. [`Vt`] is the seam other crates test through, with
//! [`FakeVt`] (feature `testing`) standing in when Zig is not available.
//!
//! The only `unsafe` in this crate is in the `ffi` module.
//!
//! # Example
//!
//! Feed bytes to a terminal and read its state back:
//!
//! ```
//! use std::sync::mpsc::channel;
//! use temor_vt::{GridSize, Terminal, TerminalConfig, Vt, VtError};
//!
//! # fn main() -> Result<(), VtError> {
//! let (events, _event_rx) = channel();
//! let (replies, _reply_rx) = channel();
//! let mut term = Terminal::new(
//!     TerminalConfig::new(GridSize::new(20, 2)),
//!     Box::new(events),
//!     Box::new(replies),
//! )?;
//! term.write(b"\x1b]2;build\x07hello");
//! assert_eq!(term.title(), "build");
//! assert_eq!(term.plain_text()?.lines().next(), Some("hello"));
//! # Ok(())
//! # }
//! ```

mod error;
mod event;
mod input;
mod model;
#[cfg(any(feature = "ghostty", feature = "testing"))]
mod scan;
mod vt;

#[cfg(feature = "testing")]
mod fake;

#[cfg(feature = "ghostty")]
#[allow(unsafe_code)]
mod ffi;

pub use error::VtError;
pub use event::{
    ClipboardAnswer, ClipboardGrant, ClipboardItem, ClipboardLocation, ClipboardRead,
    ClipboardSource, ClipboardWrite, EventSink, ExitCode, ExitStatus, Notification, Percent,
    ProgramState, ProgramStatus, ProgramWait, Progress, ProgressState, PromptEvent, PromptKind,
    RenderHold, ReplySink, Truncation, UnknownKind, VtEvent,
};
pub use input::{
    Ime, KeyAction, KeyCode, KeyInput, KittyKeyFlags, Modifiers, MouseAction, MouseButton,
    MouseFormat, MouseInput, MouseSurface, SurfacePoint,
};
pub use model::{
    AnchorId, ByteCount, CellPx, CellStyle, CellView, CellWidth, Col, ColorRef, ColorScheme, Cols,
    CursorBlink, CursorCell, CursorShape, CursorView, FrameColors, FrameDirty, Generation,
    GridPoint, GridSize, Hyperlink, ImageId, KittyLayer, KittyPixelFormat, LineCount, ModeState,
    PlacementId, PlacementIdFrom, PlacementKind, PlacementRender, PointSpace, Px, Rgb, Row,
    RowCount, RowDelta, RowDirty, RowFilter, RowId, Rows, Scrollbar, Selected, SelectionShape,
    SemanticMark, StyleFlags, TerminalConfig, TerminalMode, TextFormat, TextSelection, TextSpan,
    Underline, ViewportScroll, Visibility, Wrap, ZIndex,
};
pub use vt::Vt;

#[cfg(feature = "testing")]
pub use fake::FakeVt;

#[cfg(feature = "ghostty")]
pub use ffi::{
    Formatter, Frame, GridCell, HoldHook, KeyEncoder, KittyImage, KittyImages, KittyPixels,
    KittyPlacement, MouseEncoder, PendingFrame, RenderView, RowView, Snapshot, Terminal,
    install_png_decoder, paste_is_safe,
};
