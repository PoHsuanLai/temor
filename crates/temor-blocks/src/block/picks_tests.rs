//! Picking blocks with clicks and arrows.

use std::collections::BTreeSet;

use super::{HistoryStep, PickClick, Picks, next_bookmark};
use temor_core::vocab::ids::BlockId;

fn order() -> Vec<BlockId> {
    (1..=5).map(BlockId).collect()
}

fn ids(picks: &Picks) -> Vec<u64> {
    picks.in_order(&order()).iter().map(|b| b.0).collect()
}

#[test]
fn clicks_pick_one_a_range_or_toggle() {
    let order = order();
    let mut p = Picks::default();
    p.click(BlockId(2), PickClick::Only, &order);
    assert_eq!(ids(&p), [2]);
    p.click(BlockId(4), PickClick::Range, &order);
    assert_eq!(ids(&p), [2, 3, 4], "from the anchor to the click");
    p.click(BlockId(1), PickClick::Range, &order);
    assert_eq!(ids(&p), [1, 2], "the anchor stays where the run began");
    p.click(BlockId(5), PickClick::Toggle, &order);
    assert_eq!(ids(&p), [1, 2, 5]);
    p.click(BlockId(1), PickClick::Toggle, &order);
    assert_eq!(ids(&p), [2, 5]);
    p.click(BlockId(3), PickClick::Only, &order);
    assert_eq!(ids(&p), [3]);
    let mut fresh = Picks::default();
    fresh.click(BlockId(3), PickClick::Range, &order);
    assert_eq!(ids(&fresh), [3], "a range with no anchor is the one block");
}

#[test]
fn arrows_move_the_pick_and_past_the_newest_leave_it() {
    let order = order();
    let mut p = Picks::default();
    p.step(HistoryStep::Newer, &order);
    assert!(p.is_empty(), "nothing picked and down: still nothing");
    p.step(HistoryStep::Older, &order);
    assert_eq!(ids(&p), [5], "up from the prompt picks the newest");
    p.step(HistoryStep::Older, &order);
    assert_eq!(ids(&p), [4]);
    p.click(BlockId(1), PickClick::Only, &order);
    p.step(HistoryStep::Older, &order);
    assert_eq!(ids(&p), [1], "the oldest stays picked");
    p.click(BlockId(5), PickClick::Only, &order);
    p.step(HistoryStep::Newer, &order);
    assert!(p.is_empty(), "past the newest: back at the prompt");
}

#[test]
fn shift_arrows_grow_and_shrink_the_run() {
    let order = order();
    let mut p = Picks::default();
    p.click(BlockId(3), PickClick::Only, &order);
    p.extend(HistoryStep::Older, &order);
    p.extend(HistoryStep::Older, &order);
    assert_eq!(ids(&p), [1, 2, 3]);
    assert_eq!(p.active(), Some(BlockId(1)));
    p.extend(HistoryStep::Older, &order);
    assert_eq!(ids(&p), [1, 2, 3], "no further than the oldest");
    p.extend(HistoryStep::Newer, &order);
    assert_eq!(ids(&p), [2, 3], "back towards the anchor shrinks it");
    p.extend(HistoryStep::Newer, &order);
    p.extend(HistoryStep::Newer, &order);
    assert_eq!(ids(&p), [3, 4]);
    let mut none = Picks::default();
    none.extend(HistoryStep::Older, &order);
    assert_eq!(ids(&none), [5], "grown from nothing: the newest");
}

#[test]
fn blocks_that_are_gone_are_left_out() {
    let mut p = Picks::default();
    p.click(BlockId(2), PickClick::Only, &order());
    p.click(BlockId(3), PickClick::Toggle, &order());
    let kept: Vec<BlockId> = vec![BlockId(3), BlockId(4)];
    assert_eq!(p.in_order(&kept), [BlockId(3)]);
}

#[test]
fn select_all_picks_every_block_and_the_arrows_start_from_the_newest() {
    let order = order();
    let mut p = Picks::default();
    p.select_all(&order);
    assert_eq!(ids(&p), [1, 2, 3, 4, 5]);
    assert_eq!(p.active(), Some(BlockId(5)));
    p.extend(HistoryStep::Newer, &order);
    assert_eq!(ids(&p), [1, 2, 3, 4, 5]);
    let mut empty = Picks::default();
    empty.select_all(&[]);
    assert!(empty.is_empty());
}

#[test]
fn the_bookmark_keys_find_the_nearest_bookmark_each_way() {
    let order = order();
    let marked: BTreeSet<BlockId> = [BlockId(2), BlockId(4)].into();
    #[rustfmt::skip]
    let cases = [
        ("from the prompt, up: the newest bookmark", None, HistoryStep::Older, Some(4)),
        ("from the prompt, down: none", None, HistoryStep::Newer, None),
        ("up from a bookmark: the one above", Some(4), HistoryStep::Older, Some(2)),
        ("up from between: the one above", Some(3), HistoryStep::Older, Some(2)),
        ("down from between: the one below", Some(3), HistoryStep::Newer, Some(4)),
        ("up from the oldest bookmark: none", Some(2), HistoryStep::Older, None),
        ("down from the newest: none", Some(5), HistoryStep::Newer, None),
    ];
    for (name, from, step, want) in cases {
        let got = next_bookmark(&order, &marked, from.map(BlockId), step);
        assert_eq!(got, want.map(BlockId), "{name}");
    }
}
