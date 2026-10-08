//! Kitty images and text export, as plain values.

use ds_core::word::Word;

use super::GridPoint;

/// Which kitty placements to list, by z-layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum KittyLayer {
    All,
    /// Below the cell backgrounds (z < -1,073,741,824).
    BelowBackground,
    /// Above backgrounds, below text (z < 0).
    BelowText,
    /// Above text (z >= 0).
    AboveText,
}

/// How a kitty image's pixels are laid out. PNG payloads are always decoded to `Rgba`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum KittyPixelFormat {
    Rgb,
    Rgba,
    GrayAlpha,
    Gray,
}

/// A kitty image id (`i=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageId(pub u32);

/// A kitty placement id (`p=`); 0 when the program gave none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlacementId(pub u32);

/// A change stamp. Stamps grow monotonically across the process: an unchanged stamp means
/// unchanged content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(pub u64);

/// Whether a placement is a virtual one (Unicode placeholders) rather than drawn at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PlacementKind {
    Direct,
    Virtual,
}

/// Whether any part of a placement is in the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Visibility {
    Visible,
    Hidden,
}

/// Where and how big to draw a placement, already resolved by libghostty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacementRender {
    /// Drawn size in pixels.
    pub pixel_width: u32,
    pub pixel_height: u32,
    /// Size in cells.
    pub cols: u32,
    pub rows: u32,
    /// Top-left cell relative to the viewport; negative when scrolled partly out.
    pub viewport_col: i32,
    pub viewport_row: i32,
    pub visibility: Visibility,
    /// The source rectangle in image pixels.
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
}

/// What text export produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TextFormat {
    Plain,
    /// HTML; colours are `var(--vt-palette-N)` CSS variables.
    Html,
    /// Escape sequences that redraw the text with its styles.
    Vt,
}

/// What part of the terminal to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextSpan {
    /// Everything kept: scrollback and the live screen.
    Screen,
    /// From `start` to `end`, both included, in reading order.
    Range { start: GridPoint, end: GridPoint },
}
