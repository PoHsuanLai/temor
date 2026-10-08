//! `PaneFrame`: the renderer's own copy of a terminal's viewport, updated row by row.

use std::sync::Arc;

use temor_vt::{
    CellStyle, Col, CursorView, Generation, GridSize, ImageId, KittyLayer, Px, Rgb, Row, RowDirty,
    Selected,
};

/// A cell's grapheme cluster.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum CellText {
    Blank,
    Char(char),
    Cluster(Box<str>),
}

impl CellText {
    pub fn new(text: &str) -> Self {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (None, _) => Self::Blank,
            (Some(c), None) => Self::Char(c),
            _ => Self::Cluster(text.into()),
        }
    }

    /// The text, using `buf` for a single character.
    pub fn as_str<'a>(&'a self, buf: &'a mut [u8; 4]) -> &'a str {
        match self {
            Self::Blank => "",
            Self::Char(c) => c.encode_utf8(buf),
            Self::Cluster(s) => s,
        }
    }

    /// Whether nothing visible is drawn for the text itself.
    pub fn is_blank(&self) -> bool {
        match self {
            Self::Blank => true,
            Self::Char(c) => c.is_whitespace(),
            Self::Cluster(s) => s.chars().all(char::is_whitespace),
        }
    }
}

/// One cell worth drawing (blank default cells are left out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellDraw {
    pub col: Col,
    /// 1, or 2 for a wide character.
    pub span: u8,
    pub text: CellText,
    /// `None`: the default colour.
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub style: CellStyle,
    pub selected: Selected,
}

/// The terminal's own default colours and its first 16 palette entries, as libghostty resolved
/// them; the renderer maps them onto the UI's [`crate::Palette`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TermColors {
    pub foreground: Rgb,
    pub background: Rgb,
    /// Set by the program (OSC 12).
    pub cursor: Option<Rgb>,
    pub ansi: [Rgb; 16],
}

/// A kitty image's pixels, premultiplied RGBA8, copied once per generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImagePixels {
    pub id: ImageId,
    pub generation: Generation,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One visible placement, in cells from the viewport's top-left and in pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImagePlacement {
    pub layer: KittyLayer,
    pub image: Arc<ImagePixels>,
    pub col: i32,
    pub row: i32,
    pub width: Px,
    pub height: Px,
    /// The source rectangle in image pixels: x, y, width, height.
    pub source: [u32; 4],
}

/// The renderer's copy of a terminal's viewport. [`crate::FrameCapture`] keeps it up to date and
/// marks the rows that changed; a [`crate::Renderer`] draws those rows and clears the marks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneFrame {
    pub(crate) size: GridSize,
    pub(crate) rows: Vec<Vec<CellDraw>>,
    pub(crate) dirty: Vec<RowDirty>,
    pub(crate) cursor: Option<CursorView>,
    pub(crate) colors: TermColors,
    pub(crate) images: Vec<ImagePlacement>,
}

impl Default for PaneFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl PaneFrame {
    /// An empty 0x0 frame; the first capture fills it.
    pub fn new() -> Self {
        Self {
            size: GridSize::new(0, 0),
            rows: Vec::new(),
            dirty: Vec::new(),
            cursor: None,
            colors: TermColors::default(),
            images: Vec::new(),
        }
    }

    /// The grid the frame holds.
    pub fn size(&self) -> GridSize {
        self.size
    }

    /// The rows changed since they were last drawn.
    pub fn dirty_rows(&self) -> Vec<Row> {
        self.dirty
            .iter()
            .enumerate()
            .filter(|(_, d)| **d == RowDirty::Dirty)
            .map(|(y, _)| Row(y as u16))
            .collect()
    }

    pub(crate) fn resize(&mut self, size: GridSize) {
        if size != self.size {
            self.size = size;
            self.rows = vec![Vec::new(); usize::from(size.rows.0)];
            self.dirty = vec![RowDirty::Dirty; usize::from(size.rows.0)];
        }
    }

    pub(crate) fn set_row(&mut self, row: Row, cells: Vec<CellDraw>) {
        let y = usize::from(row.0);
        if let (Some(slot), Some(dirty)) = (self.rows.get_mut(y), self.dirty.get_mut(y)) {
            *slot = cells;
            *dirty = RowDirty::Dirty;
        }
    }

    pub(crate) fn mark_all_dirty(&mut self) {
        self.dirty.fill(RowDirty::Dirty);
    }

    pub(crate) fn set_colors(&mut self, colors: TermColors) {
        if colors != self.colors {
            self.colors = colors;
            self.mark_all_dirty();
        }
    }

    /// Clears the marks; returns the rows that were marked.
    pub(crate) fn take_dirty(&mut self) -> Vec<Row> {
        let rows = self.dirty_rows();
        self.dirty.fill(RowDirty::Clean);
        rows
    }
}
