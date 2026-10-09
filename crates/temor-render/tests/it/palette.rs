//! A new palette recolours cells that were already drawn: their colours are resolved by the
//! terminal, so the change must reach the frame.

use temor_render::PaneFocus;
use temor_vt::Rgb;

use crate::support::{PADDING, Pane, Shot};

/// The colour of the pixel at `(x, y)`.
fn pixel(shot: &Shot, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * shot.size.width.0 + x) * 4) as usize;
    [shot.rgba[i], shot.rgba[i + 1], shot.rgba[i + 2]]
}

#[test]
fn already_drawn_cells_follow_a_new_palette() {
    let mut pane = Pane::new(10, 3, 1.0, 14.0);
    // ANSI red background on two blank cells.
    pane.write(b"\x1b[41m  \x1b[0m");
    let at = (PADDING + 1, PADDING + 1);
    let before = pane.render(PaneFocus::Focused);
    let red = pane.palette.ansi[1];
    assert_eq!(pixel(&before, at.0, at.1), [red.r, red.g, red.b]);

    pane.palette.ansi[1] = Rgb::new(0x20, 0x90, 0x40);
    pane.palette.apply(&mut pane.term).expect("apply");
    let after = pane.render(PaneFocus::Focused);
    assert_eq!(pixel(&after, at.0, at.1), [0x20, 0x90, 0x40]);
}
