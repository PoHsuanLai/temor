//! With glyph atlases too small for the frame, the atlases clear while rows are built. The
//! picture must still match one drawn with room to spare: no glyph may be read from a cleared
//! atlas.

use temor_vt::{CellStyle, Col, GridSize, Rgb, Row};

use super::CpuRenderer;
use crate::frame::{CellDraw, CellText, PaneFrame, TermColors};
use crate::palette::Palette;
use crate::renderer::Renderer;
use crate::testing::{GLYPH_COLS, GLYPH_ROWS, fonts, glyph_frame, size_for, view};

const COLS: u16 = GLYPH_COLS;
const ROWS: u16 = GLYPH_ROWS;

/// One frame drawn by `renderer`: its pixels and how many atlas clears it took.
fn draw(mut renderer: CpuRenderer) -> (Vec<u8>, u32) {
    let size = size_for(&mut renderer);
    let palette = Palette::default();
    let mut f = glyph_frame(0, 160);
    let out = renderer
        .render(&mut f, &view(size, &palette))
        .expect("render");
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

/// Rows of text whose accents, descenders and wide characters reach for the rows around them.
fn lines(texts: [&str; ROWS as usize]) -> PaneFrame {
    let mut f = PaneFrame::new();
    f.resize(GridSize::new(COLS, ROWS));
    f.set_colors(TermColors {
        foreground: Rgb::new(220, 220, 220),
        background: Rgb::new(0, 0, 0),
        cursor: None,
    });
    for (r, text) in texts.into_iter().enumerate() {
        let cells = text
            .chars()
            .enumerate()
            .map(|(c, ch)| CellDraw {
                col: Col(c as u16),
                span: 1,
                text: CellText::new(&ch.to_string()),
                fg: None,
                bg: None,
                style: CellStyle::default(),
                selected: false,
            })
            .collect();
        f.set_row(Row(r as u16), cells);
    }
    f
}

#[test]
fn recomposing_rows_draws_what_a_full_compose_draws() {
    let before = [
        "Åg|Ç ÿjQ",
        "ÅÄÖ ǧ,;j",
        "gjpqy()[]",
        "日本語 é",
        "ẞ@$%&",
        "Ç",
        "",
        "g",
    ];
    let after = [
        "Åg|Ç ÿjQ",
        "ÅÄÖ ǧ,;j",
        "gjpqy()[]",
        "x",
        "ẞ@$%&",
        "Ç",
        "",
        "g",
    ];
    let mut renderer = CpuRenderer::new(fonts(14.0));
    let size = size_for(&mut renderer);
    let palette = Palette::default();
    let mut frame = lines(before);
    renderer
        .render(&mut frame, &view(size, &palette))
        .expect("first render");
    // Row 3 changes alone; the rows beside it are composed too only if its glyphs reach them.
    let next = lines(after);
    frame.set_row(Row(3), next.rows[3].clone());
    let incremental = renderer
        .render(&mut frame, &view(size, &palette))
        .expect("second render")
        .pixels
        .to_vec();
    let mut fresh = CpuRenderer::new(fonts(14.0));
    let mut whole = lines(after);
    let full = fresh
        .render(&mut whole, &view(size, &palette))
        .expect("fresh render")
        .pixels
        .to_vec();
    assert!(
        incremental == full,
        "a row recomposed alone left what a full compose would not"
    );
}

#[test]
fn a_neighbour_that_gains_an_overhang_in_the_same_frame_is_still_drawn_whole() {
    // Rows 2 and 3 change together: row 3 gains tall accents that reach up into row 2, which is
    // composed first, before row 3's overhang is known.
    let before = ["Åg|Ç", "ÅÄÖ", "gjpqy", "x", "", "", "", ""];
    let after = ["Åg|Ç", "ÅÄÖ", "y", "ÅÄÖǧ", "", "", "", ""];
    let mut renderer = CpuRenderer::new(fonts(14.0));
    let size = size_for(&mut renderer);
    let palette = Palette::default();
    let mut frame = lines(before);
    renderer
        .render(&mut frame, &view(size, &palette))
        .expect("first render");
    let next = lines(after);
    for r in [2, 3] {
        frame.set_row(Row(r), next.rows[usize::from(r)].clone());
    }
    let incremental = renderer
        .render(&mut frame, &view(size, &palette))
        .expect("second render")
        .pixels
        .to_vec();
    let mut whole = lines(after);
    let full = CpuRenderer::new(fonts(14.0))
        .render(&mut whole, &view(size, &palette))
        .expect("fresh render")
        .pixels
        .to_vec();
    assert!(
        incremental == full,
        "a row composed before its neighbour's overhang was known lost a piece of it"
    );
}

/// The rows of `texts` as a screen read hands them over.
fn styled(texts: &[&str]) -> Vec<Vec<temor_vt::StyledCell>> {
    texts
        .iter()
        .map(|text| {
            text.chars()
                .enumerate()
                .map(|(c, ch)| temor_vt::StyledCell {
                    col: Col(c as u16),
                    text: ch.to_string(),
                    width: temor_vt::CellWidth::Narrow,
                    fg: None,
                    bg: None,
                    style: CellStyle::default(),
                    hyperlink: temor_vt::Hyperlink::None,
                    semantic: temor_vt::SemanticMark::Output,
                    selected: false,
                })
                .collect()
        })
        .collect()
}

#[test]
fn a_still_draws_its_rows_as_the_live_frame_does_and_leaves_the_live_frame_alone() {
    let texts = [
        "Åg|Ç ÿjQ",
        "ÅÄÖ ǧ,;j",
        "gjpqy()[]",
        "x",
        "ẞ@$%&",
        "Ç",
        "",
        "g",
    ];
    let mut renderer = CpuRenderer::new(fonts(14.0));
    let size = size_for(&mut renderer);
    let palette = Palette::default();
    let mut live = lines(texts);
    let first = renderer
        .render(&mut live, &view(size, &palette))
        .expect("live")
        .pixels
        .to_vec();
    let still = PaneFrame::still(COLS, &styled(&texts), &live);
    let drawn = renderer
        .render_still(&still, &view(size, &palette))
        .expect("still");
    assert_eq!(drawn.size, size);
    assert!(
        drawn.pixels == first.as_slice(),
        "the still differs from the live frame"
    );
    let again = renderer
        .render(&mut live, &view(size, &palette))
        .expect("live again");
    assert!(
        again.damage.is_empty(),
        "the still made the live frame redraw"
    );
    assert!(
        again.pixels == first.as_slice(),
        "the still drew into the live frame"
    );
}
