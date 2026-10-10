//! Painting one pane (PLAN §5, §4b): the terminal's frame through the CPU renderer into
//! premultiplied pixels, handed to the pane's texture. The lock is held only for the first phase
//! of the capture; the rows are copied and drawn after it is released.

use temor_render::{
    CellMetrics, CellPos, CpuRenderer, FontSet, FrameCapture, GridFit, Padding, Palette, PaneFocus,
    PaneFrame, PaneView, PixelSize, Renderer, RowBand, Scale, SelectionOverlay, fit_grid,
};

use crate::error::UiError;
use temor_pane::PaneHandle;

/// What one paint is for: the pane's pixels, scale, focus and selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintView<'a> {
    pub pixels: PixelSize,
    pub scale: Scale,
    pub padding_logical: f32,
    pub focus: PaneFocus,
    pub selection: Option<SelectionOverlay>,
    /// The command cards over the grid's rows.
    pub bands: &'a [RowBand],
}

/// One pane's renderer, its capture state and its frame buffer.
#[derive(Debug)]
pub struct Painter {
    renderer: CpuRenderer,
    capture: FrameCapture,
    frame: PaneFrame,
    palette: Palette,
}

impl Painter {
    pub fn new(fonts: FontSet, palette: Palette) -> Result<Self, UiError> {
        Ok(Self {
            renderer: CpuRenderer::new(fonts),
            capture: FrameCapture::new()?,
            frame: PaneFrame::new(),
            palette,
        })
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
    pub fn cursor_cell(&self) -> Option<CellPos> {
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
    /// premultiplied RGBA8 pixels, with their size, to `show`. The terminal is locked only while
    /// libghostty begins the frame.
    pub fn paint(
        &mut self,
        pane: &PaneHandle,
        view: PaintView<'_>,
        show: impl FnOnce(&[u8], PixelSize),
    ) -> Result<(), UiError> {
        let Painter {
            renderer,
            capture,
            frame,
            palette,
        } = self;
        let captured = pane.with_term(|term| term.begin_frame(capture))?;
        captured.finish(frame)?;
        let pane_view = PaneView {
            size: view.pixels,
            scale: view.scale,
            padding: Padding::logical(view.padding_logical, view.scale),
            palette,
            focus: view.focus,
            selection: view.selection,
            bands: view.bands,
        };
        let drawn = renderer.render(frame, &pane_view)?;
        show(drawn.pixels, drawn.size);
        Ok(())
    }
}
