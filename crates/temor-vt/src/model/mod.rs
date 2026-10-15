//! The plain values the terminal API speaks in: sizes, points, modes, colours and the config.

mod anchor;
mod cell;
mod colour;
mod config;
mod media;
mod mode;
mod selection;
mod size;

pub use anchor::AnchorId;
pub use cell::{
    CellStyle, CellView, CellWidth, CursorBlink, CursorCell, CursorShape, CursorView, FrameColors,
    FrameDirty, Hyperlink, RowDirty, RowFilter, RowId, Selected, SemanticMark, StyleFlags,
    Underline, Wrap,
};
pub use colour::{ColorRef, ColorScheme, Rgb};
pub use config::TerminalConfig;
pub use media::{
    Generation, ImageId, KittyLayer, KittyPixelFormat, PlacementId, PlacementIdFrom, PlacementKind,
    PlacementRender, TextFormat, TextSpan, Visibility, ZIndex,
};
#[cfg(feature = "ghostty")]
pub(crate) use mode::ModeFamily;
pub use mode::{ModeState, TerminalMode};
pub use selection::{SelectionShape, TextSelection};
pub use size::{
    ByteCount, CellPx, Col, Cols, GridPoint, GridSize, LineCount, PointSpace, Px, Row, RowCount,
    RowDelta, Rows, Scrollbar, ViewportScroll,
};
