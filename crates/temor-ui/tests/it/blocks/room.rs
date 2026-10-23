//! Room under a block for a lens's picture (PLAN P11): the rows are shown around it, and the
//! card, a click and a selection still land on the rows the person sees.

use std::time::Duration;

use temor_core::block::Outcome;
use temor_pane::keys::Action;
use temor_vt::{Modifiers, MouseButton, Px, Row, SurfacePoint};

use super::super::support::tools_present;
use super::support::{Live, last_row_of, marks_of, pump, run_line, shell_path, start};

/// The middle of cell `col` of viewport row `row`, where the session shows it now.
fn shown_point(live: &Live, row: u16, col: u16) -> SurfacePoint {
    let fit = live.session.fit().expect("fitted");
    let layout = live.session.row_layout().expect("laid out");
    SurfacePoint {
        x: (fit.padding.left.0 + u32::from(col) * fit.cell.width.0 + 1) as i32,
        y: layout.row_top(Row(row)) + fit.cell.height.0 as i32 / 2,
    }
}

/// The viewport row whose text is `line`.
fn row_of(live: &Live, line: &str) -> u16 {
    let text = live.session.screen_text();
    let row = text.lines().position(|l| l.trim_end() == line);
    u16::try_from(row.unwrap_or_else(|| panic!("no {line:?} on\n{text}"))).expect("a row")
}

#[test]
fn with_room_under_a_block_the_card_a_click_and_a_selection_land_on_the_rows_shown() {
    if !tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-room", &shell);
    run_line(&live, "echo one; echo two");
    pump(&mut live, "the first block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    run_line(&live, "echo three");
    pump(&mut live, "the second block", |s| {
        s.block_outcomes()
            .get(1)
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    live.session.paint(|_, _| {}).expect("paint");
    let blocks = live.session.block_outcomes();
    let (first, second) = (blocks[0].0, blocks[1].0);
    let fit = live.session.fit().expect("fitted");
    let ch = fit.cell.height.0;
    let header_gap = |live: &Live| {
        let gutter = live.session.gutter_geometry().expect("gutter");
        let marks = marks_of(&live.session);
        let top = |id| {
            let mark = marks.iter().find(|m| m.id == id).expect("the card");
            gutter.row_top(mark.header)
        };
        top(second) - top(first)
    };
    let before = header_gap(&live);

    // Step 1: three rows of room under the first block.
    live.session.set_inserts(vec![(first, Px(3 * ch))]);
    let last = last_row_of(&live.session, first).expect("on screen");
    let layout = live.session.row_layout().expect("laid out").clone();
    let room = layout
        .insert_top(Row(u16::try_from(last).expect("a row")))
        .expect("room under the first block");
    assert_eq!(
        room,
        layout.row_top(Row(row_of(&live, "two"))) + ch as i32,
        "the room starts under the block's last row"
    );
    // Step 2: the second card sits three rows further from the first.
    let factor = live.session.scale_factor();
    assert_eq!(
        header_gap(&live),
        before + 3.0 * ch as f32 / factor,
        "the second card moved past the room"
    );
    // Step 3: a click where each block's output is shown picks that block.
    let mods = Modifiers::NONE;
    for (line, block) in [("one", first), ("three", second)] {
        let at = shown_point(&live, row_of(&live, line), 1);
        assert_eq!(live.session.block_at_point(at), Some(block), "over {line}");
        live.session
            .press(at, MouseButton::Left, mods, Duration::from_secs(10))
            .expect("press");
        live.session
            .release(at, MouseButton::Left, mods)
            .expect("release");
        assert_eq!(
            live.session.selected_block(),
            Some(block),
            "a click on {line}"
        );
    }
    // Step 4: a selection from "one" to "three", across the room, takes the rows shown.
    let from = shown_point(&live, row_of(&live, "one"), 0);
    let to = shown_point(&live, row_of(&live, "three"), 4);
    live.session
        .press(from, MouseButton::Left, mods, Duration::from_secs(20))
        .expect("press");
    live.session.drag_to(to).expect("drag");
    live.session
        .release(to, MouseButton::Left, mods)
        .expect("release");
    live.session.run(Action::Copy).expect("copy");
    let copied = live.clipboard.0.borrow().clone();
    assert!(
        copied.starts_with("one\ntwo\n") && copied.ends_with("three"),
        "copied {copied:?}"
    );
    // Step 5: a drag into the room stops at the end of the block's last row.
    live.session
        .press(from, MouseButton::Left, mods, Duration::from_secs(30))
        .expect("press");
    let into_room = SurfacePoint {
        x: from.x,
        y: room + ch as i32,
    };
    live.session.drag_to(into_room).expect("drag");
    live.session
        .release(into_room, MouseButton::Left, mods)
        .expect("release");
    live.session.run(Action::Copy).expect("copy");
    assert_eq!(live.clipboard.0.borrow().as_str(), "one\ntwo");
    // Step 6: with the room gone, every row is where it is drawn again.
    live.session.set_inserts(Vec::new());
    let layout = live.session.row_layout().expect("laid out");
    assert!(layout.gaps().is_empty(), "no room left");
    assert_eq!(header_gap(&live), before);
}
