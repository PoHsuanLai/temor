//! Only the rows that changed are composed, and only their bands are reported damaged.

use temor_render::{DamageRect, PaneFocus};
use temor_vt::Px;

use crate::support::{PADDING, Pane};

fn band(pane: &mut Pane, first: u32, rows: u32) -> DamageRect {
    let ch = pane.renderer_cell_height();
    DamageRect {
        x: Px(0),
        y: Px(PADDING + first * ch),
        width: pane.size.width,
        height: Px(rows * ch),
    }
}

impl Pane {
    fn renderer_cell_height(&mut self) -> u32 {
        use temor_render::Renderer;
        self.renderer.cell_metrics(self.scale).height()
    }
}

#[test]
fn a_change_on_one_row_recomposes_only_that_row() {
    let mut pane = Pane::new(20, 6, 1.0, 14.0);
    pane.write(b"one\r\ntwo\r\nthree\r\nfour");
    let first = pane.render(PaneFocus::Focused);
    assert_eq!(
        first.stats.rows_composed, 6,
        "the first frame draws everything"
    );
    assert_eq!(first.damage.len(), 1);

    // Unchanged: nothing composed, nothing damaged.
    let idle = pane.render(PaneFocus::Focused);
    assert_eq!(idle.stats.rows_composed, 0);
    assert!(idle.damage.is_empty());
    assert_eq!(idle.rgba, first.rgba);

    // Typing on the cursor's row touches that row only.
    pane.write(b"!");
    let typed = pane.render(PaneFocus::Focused);
    assert_eq!(typed.stats.rows_composed, 1);
    assert_eq!(typed.damage, vec![band(&mut pane, 3, 1)]);
    let (y0, y1) = (PADDING, PADDING + 3 * pane.renderer_cell_height());
    let row_bytes = (pane.size.width.0 * 4) as usize;
    let above = (y0 as usize * row_bytes)..(y1 as usize * row_bytes);
    assert_eq!(
        typed.rgba[above.clone()],
        first.rgba[above],
        "rows 0-2 untouched"
    );
}

#[test]
fn moving_the_cursor_recomposes_its_old_and_new_rows() {
    let mut pane = Pane::new(20, 6, 1.0, 14.0);
    pane.write(b"one\r\ntwo\r\nthree");
    pane.render(PaneFocus::Focused);
    pane.write(b"\x1b[1;1H");
    let moved = pane.render(PaneFocus::Focused);
    assert_eq!(moved.stats.rows_composed, 2);
    assert_eq!(
        moved.damage,
        vec![band(&mut pane, 0, 1), band(&mut pane, 2, 1)]
    );
}

#[test]
fn losing_focus_redraws_with_a_hollow_cursor() {
    let mut pane = Pane::new(10, 3, 1.0, 14.0);
    pane.write(b"$ ");
    let focused = pane.render(PaneFocus::Focused);
    let unfocused = pane.render(PaneFocus::Unfocused);
    assert_ne!(focused.rgba, unfocused.rgba);
}
