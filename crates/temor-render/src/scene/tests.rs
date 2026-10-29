use temor_vt::{CellStyle, Col, Rgb, Row, Selected, StyleFlags, Underline};

use super::SceneInput;
use super::build::build_row;
use super::colors::{UnderCursor, cell_colors};
use super::cursor::{CursorDraw, CursorStyle, resolve_cursor};
use super::model::{QuadPaint, RowScene};
use crate::font::GlyphCache;
use crate::frame::{CellDraw, CellText, TermColors};
use crate::metrics::{CellPos, Scale};
use crate::palette::Palette;
use crate::renderer::{BlinkPhase, PaneFocus, SelectionOverlay, SelectionShape};
use crate::testing::fonts;

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
    TermColors {
        foreground: Rgb::new(200, 200, 200),
        background: Rgb::new(0, 0, 0),
        cursor: None,
    }
}

#[test]
fn true_colour_cells_keep_their_colour() {
    let palette = Palette::default();
    let colors = term_colors();
    let mut c = cell(0, "x", StyleFlags::default());
    c.fg = Some(palette.ansi[1]);
    let got = cell_colors(
        &c,
        CellPos::new(0, 0),
        &colors,
        &palette,
        None,
        UnderCursor::No,
    );
    assert_eq!(got.fg, palette.ansi[1]);
    let plain = cell(0, "x", StyleFlags::default());
    let got = cell_colors(
        &plain,
        CellPos::new(0, 0),
        &colors,
        &palette,
        None,
        UnderCursor::No,
    );
    assert_eq!((got.fg, got.bg), (colors.foreground, None));
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
        (colors.background, Some(colors.foreground))
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
    assert_eq!(under.fg, colors.background);
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

#[test]
fn a_cursor_on_a_wide_blank_cell_spans_both_columns() {
    use temor_vt::{CursorBlink, CursorCell, CursorShape, CursorView};
    let view = CursorView {
        col: Col(3),
        row: Row(0),
        shape: CursorShape::Block,
        blink: CursorBlink::Steady,
        cell: CursorCell::Normal,
    };
    // The row's spans as the frame holds them: a narrow cell, then a blank wide one.
    let cursor = resolve_cursor(
        Some(view),
        |_| Some(vec![(Col(2), 1), (Col(3), 2)]),
        (PaneFocus::Focused, BlinkPhase::On),
        &Palette::default(),
        &term_colors(),
    );
    assert_eq!(cursor.map(|c| c.span), Some(2));
}

#[test]
fn a_blinking_cursor_is_left_out_only_in_its_off_half_and_only_when_focused() {
    use temor_vt::{CursorBlink, CursorCell, CursorShape, CursorView};
    // (what, the cursor's blink, focus, blink half, drawn)
    let rows = [
        (
            "blinking, on half",
            CursorBlink::Blinking,
            PaneFocus::Focused,
            BlinkPhase::On,
            true,
        ),
        (
            "blinking, off half",
            CursorBlink::Blinking,
            PaneFocus::Focused,
            BlinkPhase::Off,
            false,
        ),
        (
            "steady, off half",
            CursorBlink::Steady,
            PaneFocus::Focused,
            BlinkPhase::Off,
            true,
        ),
        (
            "unfocused never blinks",
            CursorBlink::Blinking,
            PaneFocus::Unfocused,
            BlinkPhase::Off,
            true,
        ),
    ];
    for (what, blink, focus, phase, drawn) in rows {
        let view = CursorView {
            col: Col(0),
            row: Row(0),
            shape: CursorShape::Block,
            blink,
            cell: CursorCell::Normal,
        };
        let cursor = resolve_cursor(
            Some(view),
            |_| Some(vec![(Col(0), 1)]),
            (focus, phase),
            &Palette::default(),
            &term_colors(),
        );
        assert_eq!(cursor.is_some(), drawn, "{what}");
    }
}
