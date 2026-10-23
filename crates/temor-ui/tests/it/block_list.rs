//! The pane as a list of blocks in a real window: a click on a card picks its block, and a drag
//! across cards' rows selects their text and lets go of the pick. The list's cards are found by
//! their boxes, as a person would point at them.

use std::time::Duration;

use ds::prelude::{Point, Px};
use ds_harness::Input;
use temor_ui::session::force_block_list;
use temor_ui::view::probe::Probe;

use super::drive::{Rect, rects, started, type_line, wait_html, wait_until};

/// The heads of the cards, oldest first.
fn heads(h: &ds_harness::Harness) -> Vec<Rect> {
    rects(h, ".temor-blk-head")
}

/// How many card heads say they are picked.
fn picked(html: &str) -> usize {
    html.matches("data-picked=\"true\"").count()
}

/// A point `dx` into a box from its left, halfway down.
fn left_of(r: Rect, dx: f64) -> Point {
    Point {
        x: Px((r.x + dx) as f32),
        y: Px((r.y + r.h / 2.0) as f32),
    }
}

#[test]
fn a_click_on_a_card_picks_it_and_a_drag_across_cards_selects_their_text() {
    if !super::support::window_tools_present() {
        return;
    }
    assert!(force_block_list(), "nothing asked before the test");
    let probe = Probe::new();
    let mut h = started("block-list-pointer", &probe);
    for word in ["alpha", "beta", "gamma"] {
        type_line(&mut h, &format!("echo {word}"));
        wait_until(&mut h, &probe, "the output", |s| {
            s.lines().any(|l| l.trim_end() == word)
        });
    }
    wait_html(&mut h, "three cards", |html| {
        html.matches("temor-blk-head").count() >= 3
    });
    let found = heads(&h);
    let beta = found[found.len() - 2];
    h.send(Input::click(left_of(beta, 120.0)));
    h.advance(Duration::from_millis(300));
    wait_html(&mut h, "the second card picked", |html| picked(html) == 1);
    let html = h.html();
    let at = html.find("data-picked=\"true\"").expect("a picked head");
    assert!(
        html[at..].find("echo beta") < html[at..].find("echo gamma"),
        "the picked head is beta's"
    );
    // From the start of beta's output to the end of gamma's: a selection, so no pick.
    let rows = rects(&h, ".temor-blk-rows");
    let (from, to) = (rows[rows.len() - 2], rows[rows.len() - 1]);
    h.send(Input::drag(left_of(from, 10.0), left_of(to, 60.0), 12));
    h.advance(Duration::from_millis(300));
    wait_html(&mut h, "the pick let go", |html| picked(html) == 0);
}
