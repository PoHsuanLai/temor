//! Painting one pane (PLAN §5, §4b): the terminal's frame through the CPU renderer into
//! premultiplied pixels, handed to the pane's texture. The lock is held only for the first phase
//! of the capture; the rows are copied and drawn after it is released.

use temor_pane::{PaintSource, PaneHandle};
use temor_render::{
    BlinkPhase, CapturedFrame, CellMetrics, CellPos, CpuRenderer, FontSet, FrameCapture, GridFit,
    Padding, Palette, PaneFocus, PaneFrame, PaneView, PixelSize, Renderer, RowBand, Scale,
    SelectionOverlay, fit_grid,
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
    /// The cursor's blink half now.
    pub blink: BlinkPhase,
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

/// The font the grid is drawn in, for the text drawn beside it to match: the family it was cut
/// from (the system's monospace comes to a family by name) and the size in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct GridFont {
    pub family: String,
    pub px: f32,
}

impl GridFont {
    /// CSS declarations that set text beside the grid in the grid's size and family (the family
    /// first, then the terminal's own monospace token).
    pub fn css(&self) -> String {
        format!(
            "font-family:\"{}\", var(--term-font); font-size:{}px",
            self.family.replace(['"', '\\'], ""),
            self.px
        )
    }

    fn of(fonts: &FontSet) -> Self {
        Self {
            family: fonts.resolved_family().to_owned(),
            px: fonts.size().logical(),
        }
    }
}

/// One pane's renderer, its capture state and its frame buffer.
#[derive(Debug)]
pub struct Painter {
    renderer: CpuRenderer,
    font: GridFont,
    capture: FrameCapture,
    frame: PaneFrame,
    palette: Palette,
    /// The frame of the hold in progress (valid while `showing` is `Held`).
    held: PaneFrame,
    showing: Showing,
    /// The pane's hold captures when `capture` was last started: a hold capture consumes the
    /// marks `capture` reads, so when the count moves `capture` starts over.
    seen_captures: u64,
    /// How many rows the last paint drew the grid lower than its fit: the shell's screen sits on
    /// the pane's bottom while the rows under it are empty, as Warp keeps its input at the bottom.
    lower: u16,
}

impl Painter {
    pub fn new(fonts: FontSet, palette: Palette) -> Result<Self, UiError> {
        Ok(Self {
            font: GridFont::of(&fonts),
            renderer: CpuRenderer::new(fonts),
            capture: FrameCapture::new()?,
            frame: PaneFrame::new(),
            palette,
            held: PaneFrame::new(),
            showing: Showing::Live,
            seen_captures: 0,
            lower: 0,
        })
    }

    /// The font the grid is drawn in.
    pub fn font(&self) -> &GridFont {
        &self.font
    }

    /// How many rows the last paint drew the grid lower than its fit.
    pub fn lower(&self) -> u16 {
        self.lower
    }

    /// The palette the grid is drawn with.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Changes the palette (PLAN §5: on theme change, applied to the terminal too).
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    /// Draws with `fonts` from now on (the font or its size changed). The next paint uses the new
    /// cell; the caller fits the grid to it again.
    pub fn set_fonts(&mut self, fonts: FontSet) {
        self.font = GridFont::of(&fonts);
        self.renderer = CpuRenderer::new(fonts);
    }

    /// Whether the cursor of the last paint blinks: the terminal asked for a blinking cursor
    /// and it was shown.
    pub fn cursor_blinks(&self) -> bool {
        self.frame.cursor_blinks()
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
            font: _,
            capture,
            frame,
            palette,
            held,
            showing,
            seen_captures,
            lower,
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
        // The live screen sits on the pane's bottom while its last rows are empty; a full-screen
        // program's frame fills the pane as it is.
        let source = match draw {
            Draw::Live(captured) => {
                captured.finish(frame)?;
                *lower = frame.blank_rows_below();
                Some(Showing::Live)
            }
            Draw::Held => {
                *lower = 0;
                Some(Showing::Held)
            }
            Draw::Nothing => None,
        };
        let pane_view = PaneView {
            lower: *lower,
            size: view.pixels,
            scale: view.scale,
            padding: view.inset.padding(view.scale),
            palette,
            focus: view.focus,
            blink: view.blink,
            selection: view.selection,
            bands: view.bands,
        };
        let drawn = match source {
            Some(Showing::Live) => renderer.render(frame, &pane_view)?,
            Some(Showing::Held) => renderer.render(held, &pane_view)?,
            None => return Ok(()),
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

#[cfg(test)]
mod grid_font_tests {
    use super::GridFont;

    #[test]
    fn the_css_names_the_family_first_and_never_lets_a_name_close_its_quotes() {
        let named = GridFont {
            family: "Fira \"Code\"".to_owned(),
            px: 13.5,
        };
        assert_eq!(
            named.css(),
            "font-family:\"Fira Code\", var(--term-font); font-size:13.5px"
        );
    }
}
