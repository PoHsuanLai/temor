//! The list from tables of plain reads: no terminal, no window.

use super::*;
use crate::session::frame::{Lead, Spot};
use crate::vocab::ids::ExitCode;

fn at(row: u32, col: u16) -> Spot {
    Spot { row, col }
}

/// A finished block: prompt at `first`, its command on the next row, output from the row after,
/// the next prompt starting at `end`.
fn done(id: u64, first: u32, end: u32) -> BlockRead {
    BlockRead {
        id: BlockId(id),
        command: format!("cmd{id}"),
        outcome: Outcome::Succeeded,
        took_ms: Some(4),
        prompt: Some(at(first, 0)),
        prompt_lost: false,
        output: Some(at(first + 2, 0)),
        input: Some(at(first + 1, 2)),
        end: End::At(at(end, 0)),
        lead: Lead::Header,
        picked: false,
    }
}

fn read(top: u32, len: u32, cursor: Option<u32>, blocks: Vec<BlockRead>) -> Read {
    Read {
        top,
        len,
        cursor,
        blocks,
        live: None,
    }
}

fn plain(_: BlockId) -> BlockFacts {
    BlockFacts::default()
}

fn outputs(items: &[Item]) -> Vec<(ItemKey, Range<u32>)> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Done(b) | Item::Running { block: b, .. } => Some((item.key(), b.output.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn each_finished_block_holds_the_rows_between_its_command_and_the_next_prompt() {
    let items = list_of(
        &read(0, 24, Some(9), vec![done(1, 0, 5), done(2, 5, 8)]),
        plain,
    );
    assert_eq!(
        outputs(&items),
        [
            (ItemKey::Block(BlockId(1)), 2..5),
            (ItemKey::Block(BlockId(2)), 7..8),
        ]
    );
    assert_eq!(items.last(), Some(&Item::Composer { rows: 0..0 }));
}

#[test]
fn a_block_that_printed_nothing_has_no_output_rows() {
    // `cd`: the next prompt starts on the row the output would have.
    let items = list_of(&read(0, 24, None, vec![done(1, 0, 2)]), plain);
    assert_eq!(outputs(&items), [(ItemKey::Block(BlockId(1)), 2..2)]);
}

#[test]
fn a_long_command_line_pushes_the_output_down() {
    // The command wraps over rows 1..=3; the output mark is at the start of row 4.
    let mut b = done(1, 0, 9);
    b.output = Some(at(4, 0));
    let items = list_of(&read(0, 24, None, vec![b]), plain);
    assert_eq!(outputs(&items), [(ItemKey::Block(BlockId(1)), 4..9)]);
}

#[test]
fn an_output_mark_after_the_command_text_starts_the_output_on_the_next_row() {
    let mut b = done(1, 0, 9);
    b.output = Some(at(1, 8));
    let items = list_of(&read(0, 24, None, vec![b]), plain);
    assert_eq!(outputs(&items), [(ItemKey::Block(BlockId(1)), 2..9)]);
}

#[test]
fn the_running_block_runs_to_the_cursor_and_is_live_from_the_viewport_top() {
    let mut b = done(1, 0, 0);
    b.end = End::Running;
    b.outcome = Outcome::Running;
    b.took_ms = None;
    // 100 rows of output so far, the viewport the last 24 screen rows.
    let items = list_of(&read(80, 24, Some(103), vec![b]), plain);
    match &items[..] {
        [Item::Running { block, live_from }] => {
            assert_eq!(block.output, 2..104);
            assert_eq!(*live_from, 80);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_running_block_wholly_in_view_is_live_from_its_first_row() {
    let mut b = done(1, 0, 0);
    b.end = End::Running;
    let items = list_of(&read(0, 24, Some(5), vec![b]), plain);
    assert!(matches!(&items[..], [Item::Running { live_from: 2, .. }]));
}

#[test]
fn a_folded_block_shows_no_rows_and_a_failed_one_keeps_its_outcome() {
    let mut b = done(1, 0, 6);
    b.outcome = Outcome::Failed(ExitCode(2));
    let items = list_of(&read(0, 24, None, vec![b]), |_| BlockFacts {
        folded: true,
        bookmarked: true,
    });
    match &items[0] {
        Item::Done(block) => {
            assert_eq!(block.shown_rows(), 0);
            assert!(block.bookmarked);
            assert_eq!(block.output.len(), 4);
            assert_eq!(block.outcome, Outcome::Failed(ExitCode(2)));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn blocks_with_no_command_or_lost_rows_are_left_out() {
    let mut empty = done(1, 0, 3);
    empty.command = "  ".to_owned();
    let mut lost = done(2, 3, 6);
    lost.end = End::Lost;
    let items = list_of(&read(0, 24, None, vec![empty, lost, done(3, 6, 9)]), plain);
    assert_eq!(outputs(&items), [(ItemKey::Block(BlockId(3)), 8..9)]);
}

#[test]
fn the_composer_holds_the_prompt_rows_through_the_cursor() {
    let mut r = read(0, 24, Some(7), vec![done(1, 0, 5)]);
    r.live = Some(crate::session::frame::LiveRead { prompt: at(5, 0) });
    let items = list_of(&r, plain);
    assert_eq!(items.last(), Some(&Item::Composer { rows: 5..8 }));
}

#[test]
fn a_sessions_kept_commands_become_cards_with_how_each_went() {
    use temor_schema::session::{At, RanCommand, SessionId};
    let ran = |block, command: &str, exit, ended: Option<i64>| RanCommand {
        session: SessionId::from_bits(7),
        block: BlockId(block),
        command: command.to_owned(),
        folder: "/w".to_owned(),
        exit,
        started: At(1_000),
        ended: ended.map(At),
    };
    let cards = kept_cards(&[
        ran(4, "ls", Some(0), Some(1_250)),
        ran(5, "make\n test", Some(2), None),
        ran(9, "yes", Some(130), Some(900)),
        ran(10, "true", None, Some(1_000)),
    ]);
    let seen: Vec<(u32, &str, Outcome, Option<u64>)> = cards
        .iter()
        .map(|card| (card.n, card.title(), card.outcome, card.took_ms))
        .collect();
    assert_eq!(
        seen,
        [
            (0, "ls", Outcome::Succeeded, Some(250)),
            (1, "make", Outcome::Failed(ExitCode(2)), None),
            // A clock that went back gives no time, not a wrapped one.
            (2, "yes", Outcome::Stopped(ExitCode(130)), None),
            (3, "true", Outcome::Unreported, Some(0)),
        ]
    );
    assert_eq!(cards[1].command, "make\n test");
}
