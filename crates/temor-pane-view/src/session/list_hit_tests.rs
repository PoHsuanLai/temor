//! Points over a list laid out by hand to the rows they land on.

use super::*;

fn span(top: f32, height: f32, block: u64, rows: Range<u32>, toward: Toward) -> ListSpan {
    ListSpan {
        top,
        height,
        block: Some(BlockId(block)),
        rows,
        toward,
    }
}

/// Two cards: a 40px head, rows 10..14 at 10px a row, a 10px foot, a 10px gap; then a card with
/// no output; then the composer on rows 20..21.
fn map(offset: f32) -> ListMap {
    ListMap {
        offset,
        spans: vec![
            span(0.0, 40.0, 1, 0..0, Toward::Next),
            span(40.0, 40.0, 1, 10..14, Toward::Next),
            span(80.0, 10.0, 1, 0..0, Toward::Prev),
            span(100.0, 40.0, 2, 0..0, Toward::Next),
            span(140.0, 10.0, 2, 0..0, Toward::Prev),
            ListSpan {
                top: 160.0,
                height: 10.0,
                block: None,
                rows: 20..21,
                toward: Toward::Next,
            },
        ],
    }
}

fn at(y: f32, offset: f32) -> Option<ListHit> {
    list_hit(&map(offset), y, 10.0)
}

#[test]
fn a_point_on_a_cards_rows_lands_on_the_row_it_shows() {
    let hit = at(65.0, 0.0).expect("hit");
    assert_eq!(hit.landing, Some(Landing::On(12)));
    assert_eq!(hit.block, Some(BlockId(1)));
    // Scrolled 30px down, the same row is 30px higher in the view.
    assert_eq!(
        at(35.0, 30.0).map(|h| h.landing),
        Some(Some(Landing::On(12)))
    );
}

#[test]
fn a_head_goes_with_the_rows_after_it_and_a_foot_or_gap_with_those_before() {
    assert_eq!(
        at(10.0, 0.0).map(|h| h.landing),
        Some(Some(Landing::Start(10)))
    );
    assert_eq!(
        at(85.0, 0.0).map(|h| h.landing),
        Some(Some(Landing::End(13)))
    );
    // The gap under the foot is the foot's.
    let gap = at(95.0, 0.0).expect("gap");
    assert_eq!(
        (gap.landing, gap.block),
        (Some(Landing::End(13)), Some(BlockId(1)))
    );
}

#[test]
fn a_card_with_no_output_is_its_block_and_lands_where_rows_are_near() {
    let head = at(120.0, 0.0).expect("head");
    assert_eq!(head.block, Some(BlockId(2)));
    assert_eq!(head.landing, Some(Landing::Start(20)));
    assert_eq!(
        at(145.0, 0.0).map(|h| h.landing),
        Some(Some(Landing::End(13)))
    );
}

#[test]
fn past_either_end_lands_on_the_ends_rows() {
    assert_eq!(
        at(500.0, 0.0).map(|h| h.landing),
        Some(Some(Landing::On(20)))
    );
    assert_eq!(
        at(-5.0, 0.0).map(|h| h.landing),
        Some(Some(Landing::Start(10)))
    );
    assert_eq!(list_hit(&ListMap::default(), 10.0, 10.0), None);
}

#[test]
fn a_card_in_a_list_that_draws_no_rows_is_still_its_block() {
    let map = ListMap {
        offset: 0.0,
        spans: vec![
            ListSpan {
                top: 0.0,
                height: 10.0,
                block: None,
                rows: 0..0,
                toward: Toward::Next,
            },
            span(10.0, 40.0, 1, 0..0, Toward::Next),
            span(50.0, 10.0, 1, 0..0, Toward::Prev),
            span(70.0, 40.0, 2, 0..0, Toward::Next),
        ],
    };
    let head = list_hit(&map, 20.0, 10.0).expect("the first card's head");
    assert_eq!((head.landing, head.block), (None, Some(BlockId(1))));
    assert_eq!(
        list_hit(&map, 80.0, 10.0).map(|h| h.block),
        Some(Some(BlockId(2)))
    );
    // The folder line over them is on no block.
    assert_eq!(list_hit(&map, 5.0, 10.0), None);
}
