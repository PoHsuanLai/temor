//! With glyph atlases too small for the frame, the atlases clear while rows are built. The
//! picture must still match one drawn with room to spare: no glyph may be read from a cleared
//! atlas.

use temor_vt::{CellStyle, Col, GridSize, Px, Rgb, Row, Selected};

use super::CpuRenderer;
use crate::font::testing::fonts;
use crate::frame::{CellDraw, CellText, PaneFrame, TermColors};
use crate::metrics::{Padding, PixelSize, Scale};
use crate::palette::Palette;
use crate::renderer::{PaneFocus, PaneView, Renderer};

const COLS: u16 = 40;
const ROWS: u16 = 8;

/// Box-drawing characters are drawn procedurally, one distinct mask each (160 in all), so the
/// frame needs more masks than a small atlas holds at this size.
fn frame() -> PaneFrame {
    let mut f = PaneFrame::new();
    f.resize(GridSize::new(COLS, ROWS));
    f.set_colors(TermColors {
        foreground: Rgb::new(220, 220, 220),
        background: Rgb::new(0, 0, 0),
        cursor: None,
    });
    for r in 0..ROWS {
        let cells = (0..COLS)
            .map(|c| {
                let i = usize::from(r) * usize::from(COLS) + usize::from(c);
                let text = char::from_u32(0x2500 + (i % 160) as u32)
                    .map_or_else(|| "?".to_owned(), |ch| ch.to_string());
                CellDraw {
                    col: Col(c),
                    span: 1,
                    text: CellText::new(&text),
                    fg: None,
                    bg: None,
                    style: CellStyle::default(),
                    selected: Selected::No,
                }
            })
            .collect();
        f.set_row(Row(r), cells);
    }
    f
}

/// One frame drawn by `renderer`: its pixels and how many atlas clears it took.
fn draw(mut renderer: CpuRenderer) -> (Vec<u8>, u32) {
    let scale = Scale::new(1.0);
    let m = renderer.cell_metrics(scale);
    let pad = Padding::uniform(Px(4));
    let size = PixelSize::new(
        u32::from(COLS) * m.width() + pad.left.0 * 2,
        u32::from(ROWS) * m.height() + pad.top.0 * 2,
    );
    let palette = Palette::default();
    let view = PaneView {
        size,
        scale,
        padding: pad,
        palette: &palette,
        focus: PaneFocus::Focused,
        selection: None,
    };
    let mut f = frame();
    let out = renderer.render(&mut f, &view).expect("render");
    (out.pixels.to_vec(), out.stats.atlas_resets)
}

#[test]
fn a_frame_drawn_through_atlas_clears_matches_one_drawn_with_room() {
    let (roomy, roomy_resets) = draw(CpuRenderer::new(fonts(60.0)));
    let (tight, tight_resets) = draw(CpuRenderer::with_atlas_caps(fonts(60.0), 256, 256));
    assert_eq!(roomy_resets, 0, "the default atlas holds the whole frame");
    assert!(
        tight_resets > 0,
        "the small atlas must clear for this test to mean anything"
    );
    assert!(roomy == tight, "a glyph was read from a cleared atlas");
}
