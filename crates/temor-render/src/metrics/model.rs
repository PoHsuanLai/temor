//! Units and sizes: scale, font size, pane pixels, padding, cell metrics and the fitted grid.

use temor_vt::{CellPx, Col, GridSize, Px, Row};

/// Device pixels per logical pixel (1.0, 1.5, 2.0, ...). Always finite and positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale(f32);

impl Scale {
    /// `factor`, clamped to 0.25..=8; anything not a number becomes 1.
    pub fn new(factor: f32) -> Self {
        if factor.is_finite() {
            Self(factor.clamp(0.25, 8.0))
        } else {
            Self(1.0)
        }
    }

    pub fn factor(self) -> f32 {
        self.0
    }

    /// `logical` pixels as whole device pixels, rounded to the nearest so edges stay crisp.
    pub fn device(self, logical: f32) -> Px {
        Px((logical * self.0).round().max(0.0) as u32)
    }
}

impl Default for Scale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A font size in logical pixels (the size at scale 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSize(f32);

impl FontSize {
    /// `px` logical pixels, clamped to 4..=256; anything not a number becomes 13.
    pub fn new(px: f32) -> Self {
        if px.is_finite() {
            Self(px.clamp(4.0, 256.0))
        } else {
            Self(13.0)
        }
    }

    pub fn logical(self) -> f32 {
        self.0
    }

    /// The size in device pixels at `scale`; glyphs are rasterised at this size.
    pub fn device(self, scale: Scale) -> f32 {
        self.0 * scale.factor()
    }
}

impl Default for FontSize {
    fn default() -> Self {
        Self(13.0)
    }
}

/// How tall a line is, as a multiple of the font's own line (its ascent, descent and gap), which
/// the platform may raise to a floor first. 1.0 keeps that line; 1.2 opens it by a fifth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineHeight(f32);

impl LineHeight {
    pub const MIN: f32 = 0.5;
    pub const MAX: f32 = 3.0;

    /// `factor` clamped to 0.5..=3.0; anything not a number becomes 1.0.
    pub fn new(factor: f32) -> Self {
        if factor.is_finite() {
            Self(factor.clamp(Self::MIN, Self::MAX))
        } else {
            Self(1.0)
        }
    }

    pub fn factor(self) -> f32 {
        self.0
    }
}

impl Default for LineHeight {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A size in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelSize {
    pub width: Px,
    pub height: Px,
}

impl PixelSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: Px(width),
            height: Px(height),
        }
    }
}

/// Space between the pane's edges and the grid, in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Padding {
    pub top: Px,
    pub right: Px,
    pub bottom: Px,
    pub left: Px,
}

impl Default for Padding {
    fn default() -> Self {
        Self::uniform(Px(0))
    }
}

impl Padding {
    /// The same padding on every side.
    pub fn uniform(px: Px) -> Self {
        Self {
            top: px,
            right: px,
            bottom: px,
            left: px,
        }
    }

    /// `logical` pixels on every side, snapped to whole device pixels at `scale`.
    pub fn logical(logical: f32, scale: Scale) -> Self {
        Self::uniform(scale.device(logical))
    }

    /// `[top, right, bottom, left]`, the order the mouse encoder takes.
    pub fn sides(self) -> [Px; 4] {
        [self.top, self.right, self.bottom, self.left]
    }
}

/// One cell's geometry in whole device pixels, measured from the primary font at a scale.
/// Offsets are from the cell's top edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellMetrics {
    pub cell: CellPx,
    /// The text baseline.
    pub baseline: Px,
    /// The top of a single underline.
    pub underline: Px,
    /// The top of the strikethrough line.
    pub strikeout: Px,
    /// The stroke of underlines, strikethrough and light box-drawing lines.
    pub thickness: Px,
}

impl CellMetrics {
    pub fn width(&self) -> u32 {
        self.cell.width.0
    }

    pub fn height(&self) -> u32 {
        self.cell.height.0
    }
}

/// A cell position in the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellPos {
    pub row: Row,
    pub col: Col,
}

impl CellPos {
    pub fn new(col: u16, row: u16) -> Self {
        Self {
            row: Row(row),
            col: Col(col),
        }
    }
}

/// Which half of a cell a point fell in (selection snaps by it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellSide {
    Left,
    Right,
}

/// Whether a point was over the grid or over the padding around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HitRegion {
    Grid,
    Padding,
}

/// The cell under a pixel point, clamped to the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellHit {
    pub pos: CellPos,
    pub side: CellSide,
    pub region: HitRegion,
}

/// A pane fitted with whole cells: the grid, and the padding that is left (the requested
/// padding plus the leftover pixels, which go right and bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridFit {
    pub pane: PixelSize,
    pub grid: GridSize,
    pub cell: CellPx,
    pub padding: Padding,
}
