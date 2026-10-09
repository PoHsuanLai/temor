//! Deterministic CPU renders of fixed grids, fed through a real terminal, against the PNGs in
//! tests/golden.

use temor_render::PaneFocus;

use crate::support::{Pane, assert_golden, fixture};

const FONT_PX: f32 = 14.0;

fn golden(name: &str, fixture_name: &str, (cols, rows): (u16, u16), scale: f32, focus: PaneFocus) {
    let mut pane = Pane::new(cols, rows, scale, FONT_PX);
    pane.write(&fixture(fixture_name));
    let shot = pane.render(focus);
    assert_golden(name, shot.size, &shot.rgba);
}

#[test]
fn text_and_styles() {
    golden(
        "text_styles",
        "text_styles.vt",
        (30, 6),
        1.0,
        PaneFocus::Focused,
    );
}

#[test]
fn colours() {
    golden("colours", "colours.vt", (34, 6), 1.0, PaneFocus::Focused);
}

#[test]
fn wide_characters_and_emoji() {
    golden(
        "wide_emoji",
        "wide_emoji.vt",
        (24, 5),
        1.0,
        PaneFocus::Focused,
    );
    golden(
        "wide_emoji@2x",
        "wide_emoji.vt",
        (24, 5),
        2.0,
        PaneFocus::Focused,
    );
}

#[test]
fn box_drawing() {
    golden(
        "box_drawing",
        "box_drawing.vt",
        (30, 6),
        1.0,
        PaneFocus::Focused,
    );
    golden(
        "box_drawing@2x",
        "box_drawing.vt",
        (30, 6),
        2.0,
        PaneFocus::Focused,
    );
}

#[test]
fn every_cursor_shape() {
    let grid = (12, 3);
    golden(
        "cursor_block",
        "cursor_block.vt",
        grid,
        1.0,
        PaneFocus::Focused,
    );
    golden("cursor_bar", "cursor_bar.vt", grid, 1.0, PaneFocus::Focused);
    golden(
        "cursor_underline",
        "cursor_underline.vt",
        grid,
        1.0,
        PaneFocus::Focused,
    );
    golden(
        "cursor_hollow",
        "cursor_block.vt",
        grid,
        1.0,
        PaneFocus::Unfocused,
    );
}

#[test]
fn kitty_images_on_each_layer() {
    golden(
        "kitty_layers",
        "kitty_layers.vt",
        (24, 6),
        1.0,
        PaneFocus::Focused,
    );
}

#[test]
fn one_frame_at_each_scale() {
    for (scale, name) in [(1.0, "mixed@1x"), (1.5, "mixed@1.5x"), (2.0, "mixed@2x")] {
        golden(name, "mixed.vt", (26, 4), scale, PaneFocus::Focused);
    }
}
