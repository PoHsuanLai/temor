//! A frame of box-drawing characters and the view that draws it.

use temor_vt::{CellStyle, Col, GridSize, Px, Rgb, Row};

use crate::frame::{CellDraw, CellText, PaneFrame, TermColors};
use crate::metrics::{Padding, PixelSize, Scale};
use crate::palette::Palette;
use crate::renderer::{BlinkPhase, PaneFocus, PaneView, Renderer};

/// The columns of [`glyph_frame`].
pub const GLYPH_COLS: u16 = 40;
/// The rows of [`glyph_frame`].
pub const GLYPH_ROWS: u16 = 8;
/// The padding, in device pixels, of [`view`] and [`size_for`].
pub const GLYPH_PAD: u32 = 4;

/// Cells cycling through `count` box-drawing characters from U+2500 + `first`. Box drawing is
/// drawn procedurally, one distinct mask each (160 in all), so a frame of them needs more masks
/// than a small atlas holds.
pub fn glyph_frame(first: u32, count: usize) -> PaneFrame {
    let mut f = PaneFrame::new();
    f.resize(GridSize::new(GLYPH_COLS, GLYPH_ROWS));
    f.set_colors(TermColors {
        foreground: Rgb::new(220, 220, 220),
        background: Rgb::new(0, 0, 0),
        cursor: None,
    });
    for r in 0..GLYPH_ROWS {
        let cells = (0..GLYPH_COLS)
            .map(|c| {
                let i = usize::from(r) * usize::from(GLYPH_COLS) + usize::from(c);
                let text = char::from_u32(0x2500 + first + (i % count) as u32)
                    .map_or_else(|| "?".to_owned(), |ch| ch.to_string());
                CellDraw {
                    col: Col(c),
                    span: 1,
                    text: CellText::new(&text),
                    fg: None,
                    bg: None,
                    style: CellStyle::default(),
                    selected: false,
                }
            })
            .collect();
        f.set_row(Row(r), cells);
    }
    f
}

/// The pixel size that fits [`glyph_frame`] and its padding at scale 1.
pub fn size_for(renderer: &mut impl Renderer) -> PixelSize {
    let m = renderer.cell_metrics(Scale::new(1.0));
    PixelSize::new(
        u32::from(GLYPH_COLS) * m.width() + GLYPH_PAD * 2,
        u32::from(GLYPH_ROWS) * m.height() + GLYPH_PAD * 2,
    )
}

/// A focused view of `size` at scale 1 with [`GLYPH_PAD`] padding.
pub fn view(size: PixelSize, palette: &Palette) -> PaneView<'_> {
    PaneView {
        size,
        scale: Scale::new(1.0),
        padding: Padding::uniform(Px(GLYPH_PAD)),
        palette,
        focus: PaneFocus::Focused,
        blink: BlinkPhase::On,
        selection: None,
        lower: 0,
    }
}
