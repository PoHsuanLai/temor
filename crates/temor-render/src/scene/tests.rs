use temor_vt::{CellStyle, Col, Rgb, Row, Selected, StyleFlags, Underline};

use super::colors::{UnderCursor, cell_colors, remap};
use super::cursor::{CursorDraw, CursorStyle};
use super::model::{QuadPaint, RowScene};
use super::{SceneInput, build_row};
use crate::font::GlyphCache;
use crate::font::testing::fonts;
use crate::frame::{CellDraw, CellText, TermColors};
use crate::metrics::{CellPos, Scale};
use crate::palette::Palette;
use crate::renderer::{SelectionOverlay, SelectionShape};

fn cell(col: u16, text: &str, flags: StyleFlags) -> CellDraw {
    CellDraw {
        col: Col(col),
        span: 1,
        text: CellText::new(text),
        fg: None,
        bg: None,
        style: CellStyle {
            flags,
            ..CellStyle::default()
        },
        selected: Selected::No,
    }
}

fn term_colors() -> TermColors {
    let mut ansi = [Rgb::default(); 16];
    for (i, c) in ansi.iter_mut().enumerate() {
        *c = Rgb::new(i as u8, 0, 0);
    }
    TermColors {
        foreground: Rgb::new(200, 200, 200),
        background: Rgb::new(0, 0, 0),
        cursor: None,
        ansi,
    }
}

#[test]
fn ansi_colours_map_onto_the_palette_and_others_stay() {
    let palette = Palette::default();
    let colors = term_colors();
    assert_eq!(remap(Rgb::new(1, 0, 0), &colors, &palette), palette.ansi[1]);
    assert_eq!(
        remap(Rgb::new(1, 2, 3), &colors, &palette),
        Rgb::new(1, 2, 3)
    );
}

#[test]
fn inverse_swaps_and_selection_and_cursor_win() {
    let palette = Palette::default();
    let colors = term_colors();
    let c = cell(0, "x", StyleFlags::INVERSE);
    let at = CellPos::new(0, 0);
    let inv = cell_colors(&c, at, &colors, &palette, None, UnderCursor::No);
    assert_eq!(
        (inv.fg, inv.bg),
        (palette.background, Some(palette.foreground))
    );
    let sel = SelectionOverlay {
        start: at,
        end: at,
        shape: SelectionShape::Linear,
    };
    let selected = cell_colors(&c, at, &colors, &palette, Some(sel), UnderCursor::No);
    assert_eq!(selected.bg, Some(palette.selection));
    let plain = cell(0, "x", StyleFlags::default());
    let under = cell_colors(&plain, at, &colors, &palette, None, UnderCursor::Block);
    assert_eq!(under.fg, palette.background);
}

fn build(cells: &[CellDraw], cursor: Option<CursorDraw>) -> RowScene {
    let mut glyphs = GlyphCache::new(fonts(14.0), Scale::new(1.0));
    let palette = Palette::default();
    let colors = term_colors();
    let input = SceneInput {
        metrics: glyphs.metrics(),
        origin: (4, 4),
        cols: 10,
        palette: &palette,
        colors: &colors,
        cursor,
        selection: None,
    };
    let mut out = RowScene::default();
    build_row(&input, Row(0), cells, &mut glyphs, &mut out);
    out
}

#[test]
fn a_row_puts_backgrounds_under_text_and_decorations() {
    let mut styled = cell(1, "b", StyleFlags::STRIKETHROUGH);
    styled.style.underline = Underline::Curly;
    styled.bg = Some(Rgb::new(9, 9, 9));
    let scene = build(&[cell(0, "a", StyleFlags::default()), styled], None);
    assert_eq!(scene.bg.len(), 1);
    // a, b, the curl, the strike.
    assert_eq!(scene.fg.len(), 4);
    assert!(matches!(scene.fg[0].paint, QuadPaint::Mask { .. }));
}

#[test]
fn every_cursor_style_draws() {
    for (style, bg, fg) in [
        (CursorStyle::Block, 1, 0),
        (CursorStyle::Bar, 0, 1),
        (CursorStyle::Underline, 0, 1),
        (CursorStyle::Hollow, 0, 4),
    ] {
        let cursor = CursorDraw {
            col: Col(3),
            row: Row(0),
            span: 1,
            style,
            color: Rgb::new(255, 0, 0),
        };
        let scene = build(&[], Some(cursor));
        assert_eq!((scene.bg.len(), scene.fg.len()), (bg, fg), "{style:?}");
    }
}
