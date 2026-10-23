//! What a renderer reads per frame, row and cell.

use super::{Col, ColorRef, Rgb, Row};

/// How much of the frame changed since it was last marked drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameDirty {
    /// Nothing; drawing can be skipped.
    Clean,
    /// Some rows; redraw the dirty ones.
    Partial,
    /// Everything (resize, palette, screen switch).
    Full,
}

/// Which rows a walk visits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowFilter {
    All,
    Dirty,
}

/// Whether a row changed since the frame was last marked drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowDirty {
    Dirty,
    Clean,
}

/// A row's identity across frames: the same id with a clean row means a cached row can be reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowId(pub [u64; 2]);

/// How wide a cell is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellWidth {
    Narrow,
    /// The first of two columns.
    Wide,
    /// The second column of a wide cell; draw nothing.
    SpacerTail,
    /// Padding at a line end where a wide cell wrapped to the next row; draw nothing.
    SpacerHead,
}

/// What shell integration (OSC 133) says a cell is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticMark {
    Output,
    Input,
    Prompt,
}

/// Whether a cell is part of an OSC 8 hyperlink. The address is read with
/// `Terminal::hyperlink_at`, because libghostty keeps link targets per page, not per cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hyperlink {
    None,
    Linked,
}

/// Whether a cell is inside the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Selected {
    No,
    Yes,
}

/// Underline shapes (SGR 4:x).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// SGR attributes as a bit set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StyleFlags(u16);

impl StyleFlags {
    pub const BOLD: Self = Self(1 << 0);
    pub const ITALIC: Self = Self(1 << 1);
    pub const FAINT: Self = Self(1 << 2);
    pub const BLINK: Self = Self(1 << 3);
    pub const INVERSE: Self = Self(1 << 4);
    pub const INVISIBLE: Self = Self(1 << 5);
    pub const STRIKETHROUGH: Self = Self(1 << 6);
    pub const OVERLINE: Self = Self(1 << 7);

    /// These flags plus `other`.
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every flag in `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Whether a screen row continues on the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Wrap {
    /// The row ends its line: the next row starts a new one.
    Ends,
    /// The row is soft-wrapped: the next row carries on the same line.
    Continues,
}

/// A cell's text attributes. Colours are resolved separately on [`CellView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CellStyle {
    pub flags: StyleFlags,
    pub underline: Underline,
    /// The underline colour when SGR 58 set one, resolved through the palette.
    pub underline_color: Option<Rgb>,
    /// The foreground as the program named it (palette entry or channels).
    pub fg_ref: ColorRef,
    /// The underline colour as the program named it.
    pub underline_ref: ColorRef,
}

/// One cell of a frame, borrowed for the duration of the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellView<'a> {
    pub col: Col,
    /// The full grapheme cluster; empty for a blank cell.
    pub text: &'a str,
    pub width: CellWidth,
    /// Resolved foreground; `None` means the terminal's default foreground.
    pub fg: Option<Rgb>,
    /// Resolved background; `None` means the terminal's default background.
    pub bg: Option<Rgb>,
    pub style: CellStyle,
    pub hyperlink: Hyperlink,
    pub semantic: SemanticMark,
    pub selected: Selected,
}

/// One cell of any screen row, scrollback included, owned: what a frame's [`CellView`] says of
/// a viewport cell, read for a row the viewport need not show. Nothing is selected in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledCell {
    pub col: Col,
    /// The full grapheme cluster; empty for a blank cell.
    pub text: String,
    pub width: CellWidth,
    /// Resolved foreground; `None` means the terminal's default foreground.
    pub fg: Option<Rgb>,
    /// Resolved background; `None` means the terminal's default background.
    pub bg: Option<Rgb>,
    pub style: CellStyle,
    pub hyperlink: Hyperlink,
    pub semantic: SemanticMark,
}

impl StyledCell {
    /// The cell as a frame's walk would hand it over.
    pub fn view(&self) -> CellView<'_> {
        CellView {
            col: self.col,
            text: &self.text,
            width: self.width,
            fg: self.fg,
            bg: self.bg,
            style: self.style,
            hyperlink: self.hyperlink,
            semantic: self.semantic,
            selected: Selected::No,
        }
    }
}

/// The cursor's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorShape {
    Bar,
    Block,
    Underline,
    BlockHollow,
}

/// Whether the cursor blinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorBlink {
    Steady,
    Blinking,
}

/// Whether the cursor sits on the right half of a wide cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorCell {
    Normal,
    WideTail,
}

/// The cursor as the frame shows it; absent when hidden or outside the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CursorView {
    pub col: Col,
    pub row: Row,
    pub shape: CursorShape,
    pub blink: CursorBlink,
    pub cell: CursorCell,
}

/// The frame's colours: defaults and the 256-colour palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameColors {
    pub foreground: Rgb,
    pub background: Rgb,
    /// An explicit cursor colour (OSC 12), when set.
    pub cursor: Option<Rgb>,
    pub palette: Vec<Rgb>,
}
