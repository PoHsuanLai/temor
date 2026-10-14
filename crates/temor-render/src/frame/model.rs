//! `PaneFrame`: the renderer's own copy of a terminal's viewport, updated row by row.

use std::sync::Arc;

use temor_vt::{
    CellStyle, Col, CursorView, Generation, GridSize, ImageId, KittyLayer, Px, Rgb, Row, RowDirty,
    Selected,
};

/// A cell's grapheme cluster.
// Built only by `FrameCapture` (feature `ghostty`) and tests.
#[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum CellText {
    Blank,
    Char(char),
    Cluster(Box<str>),
}

#[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
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

/// The terminal's current default colours: the host's (see [`crate::Palette::apply`]) unless the
/// program overrode them (OSC 10, 11, 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TermColors {
    pub foreground: Rgb,
    pub background: Rgb,
    /// Set by the program (OSC 12).
    pub cursor: Option<Rgb>,
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
    /// Pixel offset inside the top-left cell.
    pub offset: (u32, u32),
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

    /// The cursor's cell as of the last capture, when the cursor is shown.
    pub fn cursor_cell(&self) -> Option<crate::metrics::CellPos> {
        self.cursor
            .as_ref()
            .map(|c| crate::metrics::CellPos::new(c.col.0, c.row.0))
    }

    /// Whether the cursor is shown and the terminal asks for it to blink.
    pub fn cursor_blinks(&self) -> bool {
        self.cursor
            .as_ref()
            .is_some_and(|c| c.blink == temor_vt::CursorBlink::Blinking)
    }

    /// How many rows at the bottom of the screen are empty below everything drawn and below the
    /// cursor: nothing written in them and no colour behind them. Zero while an image is placed.
    pub fn blank_rows_below(&self) -> u16 {
        if !self.images.is_empty() {
            return 0;
        }
        let used = |row: &Vec<CellDraw>| {
            row.iter()
                .any(|cell| !cell.text.is_blank() || cell.bg.is_some())
        };
        let last_drawn = self.rows.iter().rposition(used);
        let cursor = self.cursor.as_ref().map(|c| usize::from(c.row.0));
        let lowest = match (last_drawn, cursor) {
            (None, None) => return 0,
            (a, b) => a.max(b).unwrap_or(0),
        };
        let rows = usize::from(self.size.rows.0);
        u16::try_from(rows.saturating_sub(lowest + 1)).unwrap_or(0)
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

    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
    pub(crate) fn resize(&mut self, size: GridSize) {
        if size != self.size {
            self.size = size;
            self.rows = vec![Vec::new(); usize::from(size.rows.0)];
            self.dirty = vec![RowDirty::Dirty; usize::from(size.rows.0)];
        }
    }

    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
    pub(crate) fn set_row(&mut self, row: Row, cells: Vec<CellDraw>) {
        let y = usize::from(row.0);
        if let (Some(slot), Some(dirty)) = (self.rows.get_mut(y), self.dirty.get_mut(y)) {
            *slot = cells;
            *dirty = RowDirty::Dirty;
        }
    }

    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
    pub(crate) fn mark_all_dirty(&mut self) {
        self.dirty.fill(RowDirty::Dirty);
    }

    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
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
