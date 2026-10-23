//! The block list's segments from plain lists: no session, no window.

use super::*;
use temor_core::vocab::ids::ExitCode;

const SIZES: Sizes = Sizes {
    row: 10.0,
    section: 20.0,
    head: 30.0,
    foot: 5.0,
    gap: 8.0,
    composer: 40.0,
    tile_rows: 4,
};

fn block(id: u64, output: Range<u32>) -> ListBlock {
    ListBlock {
        id: BlockId(id),
        command: format!("cmd{id}"),
        outcome: Outcome::Succeeded,
        took_ms: Some(1),
        output,
        folded: false,
        picked: false,
    }
}

fn keys(placed: &[Placed]) -> Vec<SegKey> {
    placed.iter().map(|p| p.key).collect()
}

#[test]
fn a_card_is_its_head_its_tiles_and_its_foot_then_a_gap() {
    let items = [Item::Done(block(1, 10..20)), Item::Composer { rows: 0..0 }];
    let (placed, total) = lay_out(&items, &SIZES);
    let id = BlockId(1);
    assert_eq!(
        keys(&placed),
        [
            SegKey::Head(id),
            SegKey::Tile(id, 0),
            SegKey::Tile(id, 1),
            SegKey::Tile(id, 2),
            SegKey::Foot(id),
            SegKey::Composer,
        ]
    );
    let rows: Vec<Range<u32>> = placed
        .iter()
        .filter_map(|p| match &p.seg {
            Seg::Rows { rows, .. } => Some(rows.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(rows, [10..14, 14..18, 18..20]);
    // 30 head + 100 rows + 5 foot + 8 gap + 40 composer.
    assert_eq!(total, 183.0);
    assert_eq!(placed[5].top, 143.0);
}

#[test]
fn segments_follow_on_with_no_room_between() {
    let (placed, _) = lay_out(&[Item::Done(block(1, 0..9))], &SIZES);
    for pair in placed.windows(2) {
        assert_eq!(pair[0].bottom(), pair[1].top, "{:?}", pair[1].key);
    }
}

#[test]
fn a_block_with_no_output_or_folded_is_its_head_and_foot() {
    let mut folded = block(2, 0..50);
    folded.folded = true;
    let (placed, _) = lay_out(&[Item::Done(block(1, 5..5)), Item::Done(folded)], &SIZES);
    assert_eq!(
        keys(&placed),
        [
            SegKey::Head(BlockId(1)),
            SegKey::Foot(BlockId(1)),
            SegKey::Head(BlockId(2)),
            SegKey::Foot(BlockId(2)),
        ]
    );
}

#[test]
fn the_running_block_keeps_its_settled_tiles_as_it_grows_and_shows_the_rest_live() {
    let mut b = block(1, 100..130);
    b.outcome = Outcome::Running;
    let at = |live_from: u32| {
        let (placed, _) = lay_out(
            &[Item::Running {
                block: b.clone(),
                live_from,
            }],
            &SIZES,
        );
        placed
            .into_iter()
            .filter_map(|p| match p.seg {
                Seg::Rows { rows, live, .. } => Some((p.key, rows, live)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let id = BlockId(1);
    assert_eq!(
        at(106),
        [
            (SegKey::Tile(id, 0), 100..104, false),
            (SegKey::Tile(id, 1), 104..106, false),
            (SegKey::Live(id), 106..130, true),
        ]
    );
    // Two rows later tile 0 is the same rows; tile 1 has filled.
    assert_eq!(at(108)[0], (SegKey::Tile(id, 0), 100..104, false));
    assert_eq!(at(108)[1], (SegKey::Tile(id, 1), 104..108, false));
}

#[test]
fn a_new_folder_has_its_line_and_a_failed_card_its_standing() {
    let mut failed = block(1, 0..2);
    failed.outcome = Outcome::Failed(ExitCode(1));
    let items = [
        Item::Section {
            folder: "/tmp".to_owned(),
            before: BlockId(1),
        },
        Item::Done(failed),
    ];
    let (placed, _) = lay_out(&items, &SIZES);
    assert_eq!(placed[0].seg, Seg::Section("/tmp".to_owned()));
    assert!(matches!(
        placed[2].seg,
        Seg::Rows {
            standing: Standing::Failed,
            ..
        }
    ));
}

#[test]
fn only_the_segments_near_the_view_are_mounted() {
    let items = [Item::Done(block(1, 0..400)), Item::Composer { rows: 0..0 }];
    let (placed, total) = lay_out(&items, &SIZES);
    // 100 tiles of 40px under a 30px head.
    let near = near_view(&placed, 1030.0, 100.0, 0.0);
    let first = &placed[near.start];
    let last = &placed[near.end - 1];
    assert!(first.top <= 1030.0 && first.bottom() > 1030.0);
    assert!(last.top < 1130.0 && last.bottom() >= 1130.0);
    assert_eq!(near.len(), 3);
    // At the bottom the composer is mounted.
    let near = near_view(&placed, total - 100.0, 100.0, 0.0);
    assert_eq!(placed[near.end - 1].key, SegKey::Composer);
}

#[test]
fn a_row_is_found_in_the_tile_that_draws_it() {
    let mut folded = block(2, 30..40);
    folded.folded = true;
    let items = [Item::Done(block(1, 10..20)), Item::Done(folded)];
    let (placed, _) = lay_out(&items, &SIZES);
    // Under the 30px head: tile 1 starts at row 14, 70px down; row 15 is a row further.
    assert_eq!(row_top(&placed, 15, SIZES.row), Some(80.0));
    assert_eq!(row_top(&placed, 35, SIZES.row), None, "folded away");
    assert_eq!(row_top(&placed, 99, SIZES.row), None, "in no card");
}

#[test]
fn a_long_cards_head_stays_at_the_top_until_its_end_pushes_it_away() {
    let items = [Item::Done(block(1, 0..20)), Item::Done(block(2, 20..30))];
    let (placed, _) = lay_out(&items, &SIZES);
    let cards = card_places(&placed);
    // Card 1: a 30px head, 200px of rows, a 5px foot: 0..235; card 2 from 243.
    assert_eq!((cards[0].top, cards[0].bottom), (0.0, 235.0));
    assert_eq!(sticky_head(&cards, 0.0, 30.0), None, "its own head shows");
    assert_eq!(sticky_head(&cards, 50.0, 30.0), Some((0, 0.0)));
    // 15px before the card's end the head is pushed up by what is left of 30px.
    assert_eq!(sticky_head(&cards, 190.0, 30.0), Some((0, 0.0)));
    assert_eq!(sticky_head(&cards, 220.0, 30.0), Some((0, -15.0)));
    assert_eq!(sticky_head(&cards, 240.0, 30.0), None, "between the cards");
    assert_eq!(sticky_head(&cards, 260.0, 30.0), Some((1, 0.0)));
}
