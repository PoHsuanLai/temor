//! The pointer over the block list, driven by direct calls on a real shell: the list is laid out
//! as its view lays it out, and a point over a card's rows or head lands on the screen cells and
//! the block the card shows, wherever the viewport is.

use std::time::Duration;

use temor_core::block::Outcome;
use temor_core::vocab::ids::BlockId;
use temor_pane::keys::Action;
use temor_ui::view::block_list::segments::{Placed, SegKey, Sizes, lay_out, list_map};
use temor_vt::{Modifiers, MouseButton, SurfacePoint};

use super::super::support::tools_present;
use super::support::{Live, pump, run_line, shell_path, start};

/// A bash with three finished blocks, `echo a`, `echo b` and `echo c`, oldest first, and the
/// list laid out over it, scrolled to its top.
fn listed(name: &str) -> Option<(Live, [BlockId; 3], Vec<Placed>)> {
    if !tools_present() {
        return None;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start(name, &shell);
    for word in ["a", "b", "c"] {
        let before = live.session.block_outcomes().len();
        run_line(&live, &format!("echo {word}"));
        pump(&mut live, "a finished block", |s| {
            s.block_outcomes().len() > before
                && s.block_outcomes()
                    .last()
                    .is_some_and(|b| b.1 != Outcome::Running)
        });
    }
    let ids: Vec<BlockId> = live
        .session
        .block_outcomes()
        .into_iter()
        .map(|b| b.0)
        .collect();
    let row = live.session.row_height().expect("laid out");
    let sizes = Sizes {
        row,
        head: 40.0,
        foot: 10.0,
        gap: 10.0,
        composer: row,
        tile_rows: 48,
    };
    let items = live.session.block_list().expect("a list");
    let (placed, _) = lay_out(&items, &sizes, &[]);
    live.session.set_list_map(Some(list_map(&placed, 0.0)));
    Some((live, [ids[0], ids[1], ids[2]], placed))
}

/// A point `dy` logical pixels into segment `key`, over column `col`.
fn over(live: &Live, placed: &[Placed], key: SegKey, col: u16, dy: f32) -> SurfacePoint {
    let seg = placed.iter().find(|p| p.key == key).expect("the segment");
    let fit = live.session.fit().expect("fitted");
    let (x, _) = fit.cell_origin(temor_render::CellPos::new(col, 0));
    let factor = live.session.scale_factor();
    SurfacePoint {
        x: x.0 as i32 + 2,
        y: ((seg.top + dy) * factor) as i32,
    }
}

fn click(live: &mut Live, at: SurfacePoint, clock: &mut u64) {
    *clock += 5;
    let now = Duration::from_secs(*clock);
    live.session
        .press(at, MouseButton::Left, Modifiers::NONE, now)
        .expect("press");
    live.session
        .release(at, MouseButton::Left, Modifiers::NONE)
        .expect("release");
}

fn copied(live: &mut Live) -> String {
    *live.clipboard.0.borrow_mut() = String::new();
    live.session.run(Action::Copy).expect("copy");
    live.clipboard.0.borrow().clone()
}

#[test]
fn a_click_on_a_cards_rows_or_head_picks_its_block() {
    let Some((mut live, [a, b, _], placed)) = listed("list-click") else {
        return;
    };
    let mut clock = 0;
    let half = live.session.row_height().expect("row") / 2.0;
    let on_b = over(&live, &placed, SegKey::Tile(b, 0), 0, half);
    click(&mut live, on_b, &mut clock);
    assert_eq!(live.session.picked_blocks(), [b], "the card under the rows");
    let on_a = over(&live, &placed, SegKey::Head(a), 3, 20.0);
    click(&mut live, on_a, &mut clock);
    assert_eq!(live.session.picked_blocks(), [a], "the card under the head");
    let on_prompt = over(&live, &placed, SegKey::Composer, 1, half);
    click(&mut live, on_prompt, &mut clock);
    assert!(
        live.session.picked_blocks().is_empty(),
        "the prompt lets go"
    );
}

#[test]
fn a_drag_over_a_cards_rows_selects_the_text_they_show() {
    let Some((mut live, [_, b, c], placed)) = listed("list-drag") else {
        return;
    };
    let half = live.session.row_height().expect("row") / 2.0;
    let from = over(&live, &placed, SegKey::Tile(b, 0), 0, half);
    let to = over(&live, &placed, SegKey::Tile(c, 0), 1, half);
    live.session
        .press(
            from,
            MouseButton::Left,
            Modifiers::NONE,
            Duration::from_secs(9),
        )
        .expect("press");
    live.session.drag_to(to).expect("drag");
    live.session
        .release(to, MouseButton::Left, Modifiers::NONE)
        .expect("release");
    let text = copied(&mut live);
    assert!(text.starts_with('b'), "from b's output: {text:?}");
    assert!(text.ends_with('c'), "to c's output: {text:?}");
}
