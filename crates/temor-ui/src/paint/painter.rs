//! Painting one pane (PLAN §5, §4b): the terminal's frame through the CPU renderer into
//! premultiplied pixels, handed to the pane's texture. The lock is held only for the first phase
//! of the capture; the rows are copied and drawn after it is released.

use temor_pane::{PaintSource, PaneHandle};
use temor_render::{
    CapturedFrame, CellMetrics, CellPos, CpuRenderer, FontSet, FrameCapture, GridFit, Padding,
    Palette, PaneFocus, PaneFrame, PaneView, PixelSize, Renderer, RowBand, Scale, SelectionOverlay,
    fit_grid,
};

use crate::error::UiError;

/// The space between a pane's edges and its grid, in logical pixels: the same on the left and the
/// right, and the same on the top and the bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inset {
    pub horizontal: f32,
    pub vertical: f32,
}

impl Inset {
    /// The inset as device pixels at `scale`, snapped to whole pixels.
    fn padding(self, scale: Scale) -> Padding {
        let (x, y) = (scale.device(self.horizontal), scale.device(self.vertical));
        Padding {
            top: y,
            right: x,
            bottom: y,
            left: x,
        }
    }
}

/// What one paint is for: the pane's pixels, scale, focus and selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintView<'a> {
    pub pixels: PixelSize,
    pub scale: Scale,
    pub inset: Inset,
    pub focus: PaneFocus,
    pub selection: Option<SelectionOverlay>,
    /// The command cards over the grid's rows.
    pub bands: &'a [RowBand],
}

/// Which frame the pane's pixels were last drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    /// The terminal, through `capture`.
    Live,
    /// The frame captured when the program began holding the screen.
    Held,
}

/// What one paint draws from.
enum Draw<'c> {
    Live(CapturedFrame<'c>),
    Held,
    Nothing,
}

/// One pane's renderer, its capture state and its frame buffer.
#[derive(Debug)]
pub struct Painter {
    renderer: CpuRenderer,
    capture: FrameCapture,
    frame: PaneFrame,
    palette: Palette,
    /// The frame of the hold in progress (valid while `showing` is `Held`).
    held: PaneFrame,
    showing: Showing,
    /// The pane's hold captures when `capture` was last started: a hold capture consumes the
    /// marks `capture` reads, so when the count moves `capture` starts over.
    seen_captures: u64,
}

impl Painter {
    pub fn new(fonts: FontSet, palette: Palette) -> Result<Self, UiError> {
        Ok(Self {
            renderer: CpuRenderer::new(fonts),
            capture: FrameCapture::new()?,
            frame: PaneFrame::new(),
            palette,
            held: PaneFrame::new(),
            showing: Showing::Live,
            seen_captures: 0,
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
    pub fn fit(&mut self, pixels: PixelSize, scale: Scale, inset: Inset) -> GridFit {
        let metrics = self.renderer.cell_metrics(scale);
        fit_grid(pixels, &metrics, inset.padding(scale))
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
            held,
            showing,
            seen_captures,
        } = self;
        // Decided and begun under one hold of the pane's lock, so a frame is never begun in the
        // middle of a hold: while the program holds the screen the captured frame is drawn.
        let draw = pane.with_term(|term| {
            Ok::<_, UiError>(match term.paint_source() {
                PaintSource::Live { captures } => {
                    if *showing == Showing::Held || *seen_captures != captures {
                        // A full redraw: the marks this capture would read were consumed.
                        *capture = FrameCapture::new()?;
                        *seen_captures = captures;
                    }
                    *showing = Showing::Live;
                    Draw::Live(term.begin_frame(capture)?)
                }
                PaintSource::Fresh(fresh) => {
                    *held = *fresh;
                    *showing = Showing::Held;
                    Draw::Held
                }
                PaintSource::Held if *showing == Showing::Held => Draw::Held,
                _ => Draw::Nothing,
            })
        })?;
        let pane_view = PaneView {
            size: view.pixels,
            scale: view.scale,
            padding: view.inset.padding(view.scale),
            palette,
            focus: view.focus,
            selection: view.selection,
            bands: view.bands,
        };
        let drawn = match draw {
            Draw::Live(captured) => {
                captured.finish(frame)?;
                renderer.render(frame, &pane_view)?
            }
            Draw::Held => renderer.render(held, &pane_view)?,
            Draw::Nothing => return Ok(()),
        };
        show(drawn.pixels, drawn.size);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Inset;
    use temor_render::Scale;

    #[test]
    fn the_grid_starts_16_px_in_from_the_sides_and_8_from_the_top_at_every_scale() {
        // (scale, left and right, top and bottom) in device pixels.
        let cases = [(1.0, 16, 8), (1.5, 24, 12), (2.0, 32, 16)];
        for (factor, across, down) in cases {
            let pad = Inset {
                horizontal: 16.0,
                vertical: 8.0,
            }
            .padding(Scale::new(factor));
            let got = [pad.left.0, pad.right.0, pad.top.0, pad.bottom.0];
            assert_eq!(got, [across, across, down, down], "scale {factor}");
        }
    }
}
