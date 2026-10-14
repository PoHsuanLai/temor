//! The `Renderer` seam: what a pane is drawn for, and what comes back.

use temor_vt::Px;

use crate::bands::RowBand;
use crate::error::RenderError;
use crate::frame::PaneFrame;
use crate::metrics::{CellMetrics, CellPos, Padding, PixelSize, Scale};
use crate::palette::Palette;

/// Whether the pane has keyboard focus; an unfocused pane shows a hollow cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneFocus {
    Focused,
    Unfocused,
}

/// Which half of its blink the cursor is in. Only a blinking cursor in a focused pane is left
/// out in the `Off` half; any other cursor is drawn in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlinkPhase {
    #[default]
    On,
    Off,
}

/// How a selection covers cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionShape {
    /// Reading order from `start` to `end`.
    Linear,
    /// The rectangle with `start` and `end` as corners.
    Block,
}

/// A selection the UI holds, drawn over the cells (both ends included). Cells libghostty marks
/// selected are drawn the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectionOverlay {
    pub start: CellPos,
    pub end: CellPos,
    pub shape: SelectionShape,
}

impl SelectionOverlay {
    /// Whether `pos` is inside.
    pub fn contains(&self, pos: CellPos) -> bool {
        let (a, b) = if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };
        match self.shape {
            SelectionShape::Linear => a <= pos && pos <= b,
            SelectionShape::Block => {
                let (c0, c1) = (a.col.min(b.col), a.col.max(b.col));
                a.row <= pos.row && pos.row <= b.row && c0 <= pos.col && pos.col <= c1
            }
        }
    }
}

/// What a pane is drawn for: its device-pixel size, the scale, the padding to keep, the colours,
/// focus, any selection and the row bands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneView<'a> {
    pub size: PixelSize,
    pub scale: Scale,
    /// The least padding around the grid; leftover pixels widen it right and bottom.
    pub padding: Padding,
    pub palette: &'a Palette,
    pub focus: PaneFocus,
    /// The cursor's blink half now.
    pub blink: BlinkPhase,
    pub selection: Option<SelectionOverlay>,
    /// Fills, stripes and rules over runs of viewport rows (command cards); usually empty.
    pub bands: &'a [RowBand],
    /// How many rows the grid is drawn lower than its fit, so its last drawn row meets the
    /// pane's bottom while the screen below it is empty (see [`PaneFrame::blank_rows_below`]).
    pub lower: u16,
}

/// A changed rectangle of the output, in device pixels from its top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DamageRect {
    pub x: Px,
    pub y: Px,
    pub width: Px,
    pub height: Px,
}

/// What one render did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RenderStats {
    /// Grid rows composed this frame.
    pub rows_composed: u32,
    /// Glyph-atlas clears this frame (the atlas filled up).
    pub atlas_resets: u32,
    /// Kitty images not drawn this frame: empty, or larger than the device's texture limit
    /// (GPU only; the CPU path has no such limit).
    pub images_skipped: u32,
}

/// Turns a [`PaneFrame`] into pixels for a pane. Two implementations: [`crate::CpuRenderer`]
/// (premultiplied RGBA8 in memory, the default) and, with feature `gpu`, `GpuRenderer`
/// (a texture on the window's device).
pub trait Renderer {
    /// What a render hands back: borrowed pixels or a texture, with the damaged rectangles.
    type Output<'a>
    where
        Self: 'a;

    /// The cell at `scale`, for sizing the terminal (`Terminal::resize`) before drawing.
    fn cell_metrics(&mut self, scale: Scale) -> CellMetrics;

    /// Draws what changed in `frame` since the last render (everything when the view's size,
    /// scale or colours changed) and clears the frame's dirty marks.
    fn render(
        &mut self,
        frame: &mut PaneFrame,
        view: &PaneView<'_>,
    ) -> Result<Self::Output<'_>, RenderError>;
}
