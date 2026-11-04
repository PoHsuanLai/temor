//! temor's terminal core. `Terminal` wraps libghostty-vt (feature `ghostty`, on by default):
//! bytes in, a typed [`VtEvent`] stream out through a caller-supplied [`EventSink`], and a
//! two-phase [`RenderView`] for the renderer. Without `ghostty` the crate is the terminal's
//! vocabulary alone (sizes, cells, keys, events), which builds without Zig.
//!
//! The only `unsafe` in this crate is in the `ffi` module.
//!
//! # Example
//!
//! Feed bytes to a terminal and read its state back:
//!
//! ```
//! use std::sync::mpsc::channel;
//! use temor_vt::{GridSize, Terminal, TerminalConfig, VtError};
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
#[cfg(feature = "ghostty")]
mod scan;

#[cfg(feature = "ghostty")]
#[allow(unsafe_code)]
mod ffi;

pub use error::VtError;
pub use event::{
    ClipboardAnswer, ClipboardItem, ClipboardLocation, ClipboardRead, ClipboardSource,
    ClipboardWrite, EventSink, ExitCode, ExitStatus, Notification, Percent, ProgramState,
    ProgramStatus, ProgramWait, Progress, ProgressState, PromptEvent, PromptKind, RenderHold,
    ReplySink, UnknownKind, VtEvent,
};
pub use input::{
    KeyAction, KeyCode, KeyInput, KittyKeyFlags, Modifiers, MouseAction, MouseButton, MouseFormat,
    MouseInput, MouseSurface, OptionAsAlt, SurfacePoint,
};
pub use model::{
    AnchorId, ByteCount, CellPx, CellStyle, CellView, CellWidth, Col, ColorRef, ColorScheme, Cols,
    CursorShape, CursorView, FrameColors, FrameDirty, Generation, GridPoint, GridSize, ImageId,
    KittyLayer, KittyPixelFormat, LineCount, ModeState, PlacementId, PlacementIdFrom,
    PlacementKind, PlacementRender, PointSpace, Px, Rgb, Row, RowCount, RowDelta, RowFilter, RowId,
    Rows, Scrollbar, SelectionShape, SemanticMark, StyleFlags, StyledCell, TerminalConfig,
    TerminalMode, TextFormat, TextSelection, TextSpan, Underline, ViewportScroll, Wrap, ZIndex,
};

#[cfg(feature = "ghostty")]
pub use ffi::{
    Formatter, Frame, GridCell, HoldHook, KeyEncoder, KittyImage, KittyImages, KittyPixels,
    KittyPlacement, MouseEncoder, PendingFrame, RenderView, RowView, Snapshot, Terminal,
    install_png_decoder, paste_is_safe,
};
