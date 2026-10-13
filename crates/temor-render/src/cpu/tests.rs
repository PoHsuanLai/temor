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
        blink: crate::renderer::BlinkPhase::On,
        selection: None,
        bands: &[],
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

mod bands {
    use temor_vt::{GridSize, Px, Rgb, Row};

    use super::{COLS, CpuRenderer, ROWS, fonts};
    use crate::bands::{Alpha, Cut, RowBand, RowSpan, RowWeight, Tint};
    use crate::frame::{PaneFrame, TermColors};
    use crate::metrics::{Padding, PixelSize, Scale};
    use crate::palette::Palette;
    use crate::renderer::{PaneFocus, PaneView, Renderer};

    const PAD: u32 = 4;

    struct Bench {
        renderer: CpuRenderer,
        frame: PaneFrame,
        palette: Palette,
        size: PixelSize,
        cell_h: u32,
    }

    impl Bench {
        fn new() -> Self {
            let mut renderer = CpuRenderer::new(fonts(14.0));
            let m = renderer.cell_metrics(Scale::new(1.0));
            let mut frame = PaneFrame::new();
            frame.resize(GridSize::new(COLS, ROWS));
            frame.set_colors(TermColors {
                foreground: Rgb::new(220, 220, 220),
                background: Rgb::new(0, 0, 0),
                cursor: None,
            });
            Self {
                renderer,
                frame,
                palette: Palette::default(),
                size: PixelSize::new(
                    u32::from(COLS) * m.width() + PAD * 2,
                    u32::from(ROWS) * m.height() + PAD * 2,
                ),
                cell_h: m.height(),
            }
        }

        /// Renders with `bands`: the pixels and the number of rows composed.
        fn render(&mut self, bands: &[RowBand]) -> (Vec<u8>, u32) {
            let view = PaneView {
                size: self.size,
                scale: Scale::new(1.0),
                padding: Padding::uniform(Px(PAD)),
                palette: &self.palette,
                focus: PaneFocus::Focused,
                blink: crate::renderer::BlinkPhase::On,
                selection: None,
                bands,
            };
            let out = self
                .renderer
                .render(&mut self.frame, &view)
                .expect("render");
            (out.pixels.to_vec(), out.stats.rows_composed)
        }

        fn px(&self, pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
            let i = ((y * self.size.width.0 + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        }

        /// The top pixel row of grid row `r`.
        fn top(&self, r: u32) -> u32 {
            PAD + r * self.cell_h
        }
    }

    fn tint(r: u8, g: u8, b: u8, a: u8) -> Option<Tint> {
        Some(Tint {
            color: Rgb::new(r, g, b),
            alpha: Alpha(a),
        })
    }

    fn band(row: u16, fill: Option<Tint>, stripe: Option<Tint>, rule: Option<Tint>) -> RowBand {
        RowBand {
            rows: RowSpan {
                first: Row(row),
                last: Row(row),
            },
            top: Cut::Edge,
            bottom: Cut::Edge,
            fill,
            stripe,
            rule,
            weight: RowWeight::Normal,
        }
    }

    #[test]
    fn a_fill_tints_its_row_across_the_padding_and_not_the_row_below() {
        let mut b = Bench::new();
        let (px, _) = b.render(&[band(1, tint(200, 0, 0, 128), None, None)]);
        let mid = b.top(1) + b.cell_h / 2;
        // (x, y, red): inside the grid, in the left padding, in the right padding, row below.
        for (x, y, red) in [
            (40, mid, 100),
            (1, mid, 100),
            (b.size.width.0 - 1, mid, 100),
            (40, b.top(2) + b.cell_h / 2, 0),
            (40, b.top(0) + b.cell_h / 2, 0),
        ] {
            assert_eq!(b.px(&px, x, y)[0], red, "red at ({x}, {y})");
        }
    }

    #[test]
    fn the_stripe_lies_inside_the_left_padding() {
        let mut b = Bench::new();
        let (px, _) = b.render(&[band(1, None, tint(0, 255, 0, 255), None)]);
        let mid = b.top(1) + b.cell_h / 2;
        // Three logical px at scale 1: columns 0..3 are the stripe, the rest of the padding is not.
        for (x, green) in [(0, 255), (2, 255), (3, 0), (PAD - 1, 0), (PAD + 10, 0)] {
            assert_eq!(b.px(&px, x, mid)[1], green, "green at x {x}");
        }
        assert_eq!(b.px(&px, 1, b.top(2) + 1)[1], 0, "the row below has none");
    }

    #[test]
    fn the_rule_is_one_pixel_on_the_top_edge() {
        let mut b = Bench::new();
        let (px, _) = b.render(&[band(2, None, None, tint(0, 0, 255, 255))]);
        let top = b.top(2);
        for (y, blue) in [(top - 1, 0), (top, 255), (top + 1, 0)] {
            for x in [0, 40, b.size.width.0 - 1] {
                assert_eq!(b.px(&px, x, y)[2], blue, "blue at ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_band_cut_across_middles_paints_and_rules_from_the_middle() {
        let mut b = Bench::new();
        let mut card = band(1, tint(200, 0, 0, 255), None, tint(0, 0, 255, 255));
        card.rows.last = Row(3);
        (card.top, card.bottom) = (Cut::Middle, Cut::Middle);
        let (px, _) = b.render(&[card]);
        let half = b.cell_h / 2;
        let (rule, end) = (b.top(1) + half, b.top(3) + half);
        // (y, red, blue): above the rule, the rule, below it, the middle row, the last row's
        // upper half, its lower half.
        for (y, red, blue) in [
            (rule - 1, 0, 0),
            (rule, 0, 255),
            (rule + 1, 200, 0),
            (b.top(2) + half, 200, 0),
            (end - 1, 200, 0),
            (end, 0, 0),
        ] {
            let at = b.px(&px, 40, y);
            assert_eq!((at[0], at[2]), (red, blue), "red and blue at y {y}");
        }
    }

    #[test]
    fn a_changed_band_redraws_exactly_its_rows() {
        let mut b = Bench::new();
        let first = band(1, tint(200, 0, 0, 128), None, None);
        let moved = band(3, tint(200, 0, 0, 128), None, None);
        let (_, all) = b.render(&[first]);
        assert_eq!(all, u32::from(ROWS), "the first frame draws everything");
        // (bands, rows composed)
        for (bands, composed) in [(vec![first], 0), (vec![moved], 2), (vec![], 1), (vec![], 0)] {
            let (_, rows) = b.render(&bands);
            assert_eq!(rows, composed);
        }
    }
}
