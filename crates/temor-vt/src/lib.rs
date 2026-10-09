//! temor's terminal core. `Terminal` wraps libghostty-vt (feature `ghostty`, on by default):
//! bytes in, a typed [`VtEvent`] stream out through a caller-supplied [`EventSink`], and a
//! two-phase [`RenderView`] for the renderer. [`Vt`] is the seam other crates test through, with
//! [`FakeVt`] (feature `fake`) standing in when Zig is not available.
//!
//! The only `unsafe` in this crate is in the `ffi` module.

mod error;
mod event;
mod input;
mod model;
#[cfg(any(feature = "ghostty", feature = "fake"))]
mod scan;
mod vt;

#[cfg(feature = "fake")]
mod fake;

#[cfg(feature = "ghostty")]
#[allow(unsafe_code)]
mod ffi;

pub use error::VtError;
pub use event::{
    ClipboardGrant, ClipboardItem, ClipboardLocation, ClipboardWrite, EventSink, ExitCode,
    ExitStatus, Notification, Percent, ProgramState, ProgramStatus, ProgramWait, Progress,
    ProgressState, PromptEvent, PromptKind, RenderHold, ReplySink, Truncation, UnknownKind,
    VtEvent,
};
pub use input::{
    Ime, KeyAction, KeyCode, KeyInput, KittyKeyFlags, Modifiers, MouseAction, MouseButton,
    MouseFormat, MouseInput, MouseSurface, SurfacePoint,
};
pub use model::{
    ByteCount, CellPx, CellStyle, CellView, CellWidth, Col, ColorScheme, Cols, CursorBlink,
    CursorCell, CursorShape, CursorView, FrameColors, FrameDirty, Generation, GridPoint, GridSize,
    Hyperlink, ImageId, KittyLayer, KittyPixelFormat, LineCount, ModeState, PlacementId,
    PlacementKind, PlacementRender, PointSpace, Px, Rgb, Row, RowCount, RowDelta, RowDirty,
    RowFilter, RowId, Rows, Scrollbar, Selected, SelectionShape, SemanticMark, StyleFlags,
    TerminalConfig, TerminalMode, TextFormat, TextSelection, TextSpan, Underline, ViewportScroll,
    Visibility, ZIndex,
};
pub use vt::Vt;

#[cfg(feature = "fake")]
pub use fake::FakeVt;

#[cfg(feature = "ghostty")]
pub use ffi::{
    Formatter, Frame, GridCell, KeyEncoder, KittyImage, KittyImages, KittyPixels, KittyPlacement,
    MouseEncoder, PendingFrame, RenderView, RowView, Snapshot, Terminal, install_png_decoder,
    paste_is_safe,
};
