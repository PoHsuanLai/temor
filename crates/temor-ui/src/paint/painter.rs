//! Painting one pane (PLAN §5): the terminal's frame through the CPU renderer into premultiplied
//! pixels, handed to the pane's texture. The renderer keeps its glyph atlases between frames.

use temor_render::{
    CellMetrics, CpuRenderer, FontSet, GridFit, Padding, Palette, PaneFocus, PaneFrame, PaneView,
    PixelSize, Renderer, Scale, SelectionOverlay, fit_grid,
};

use crate::error::UiError;
use crate::pane::Term;

/// One pane's renderer and its frame buffer.
#[derive(Debug)]
pub struct Painter {
    renderer: CpuRenderer,
    frame: PaneFrame,
    palette: Palette,
}

impl Painter {
    pub fn new(fonts: FontSet, palette: Palette) -> Self {
        Self {
            renderer: CpuRenderer::new(fonts),
            frame: PaneFrame::new(),
            palette,
        }
    }

    /// The palette the grid is drawn with.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Changes the palette (PLAN §5: on theme change, applied to the terminal too).
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    /// The cursor's cell as of the last paint, when the cursor is shown.
    pub fn cursor_cell(&self) -> Option<temor_render::CellPos> {
        self.frame.cursor_cell()
    }

    /// The cell at `scale`.
    pub fn metrics(&mut self, scale: Scale) -> CellMetrics {
        self.renderer.cell_metrics(scale)
    }

    /// The grid that fits a pane of `pixels` at `scale`, with the style's padding.
    pub fn fit(&mut self, pixels: PixelSize, scale: Scale, padding_logical: f32) -> GridFit {
        let metrics = self.renderer.cell_metrics(scale);
        fit_grid(pixels, &metrics, Padding::logical(padding_logical, scale))
    }

    /// Draws what changed in the terminal since the last frame and hands the whole pane's
    /// premultiplied RGBA8 pixels, with their size, to `show`.
    pub fn paint(
        &mut self,
        term: &mut Term,
        view: PaintView,
        show: impl FnOnce(&[u8], PixelSize),
    ) -> Result<(), UiError> {
        term.capture(&mut self.frame)?;
        let pane = PaneView {
            size: view.pixels,
            scale: view.scale,
            padding: Padding::logical(view.padding_logical, view.scale),
            palette: &self.palette,
            focus: view.focus,
            selection: view.selection,
        };
        let frame = self.renderer.render(&mut self.frame, &pane)?;
        show(frame.pixels, frame.size);
        Ok(())
    }
}

/// What one paint is for: the pane's pixels, scale, focus and selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintView {
    pub pixels: PixelSize,
    pub scale: Scale,
    pub padding_logical: f32,
    pub focus: PaneFocus,
    pub selection: Option<SelectionOverlay>,
}
