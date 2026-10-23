//! The frame's answers from tables of plain reads: no terminal, no session.

use super::{BlockRead, End, Frame, LiveRead, Read, Spot};
use crate::block::Outcome;
use crate::vocab::ids::{BlockId, ExitCode};

use super::super::card::{Lead, Mark, Pick, Stage};

fn at(row: u32, col: u16) -> Spot {
    Spot { row, col }
}

/// A finished block: prompt at `first`, its command on the next row, output after, ending at
/// `end` (a line's start).
fn done(id: u64, first: u32, end: u32) -> BlockRead {
    BlockRead {
        id: BlockId(id),
        command: format!("cmd{id}"),
        outcome: Outcome::Succeeded,
        took_ms: Some(4),
        prompt: Some(at(first, 0)),
        output: Some(at(first + 2, 0)),
        input: Some(at(first + 1, 2)),
        end: End::At(at(end, 0)),
        lead: Lead::Header,
        header_cols: 7,
        picked: false,
        bookmarked: false,
    }
}

fn read(top: u32, len: u32, blocks: Vec<BlockRead>) -> Read {
    Read {
        top,
        len,
        cursor: None,
        first_shown: 0,
        blocks,
        live: None,
    }
}

#[test]
fn marks_are_the_blocks_whose_prompts_are_in_the_viewport() {
    let mut blank = done(2, 10, 15);
    blank.lead = Lead::Blank;
    blank.outcome = Outcome::Failed(ExitCode(1));
    let frame = Frame::of(read(8, 10, vec![done(1, 2, 7), blank, done(3, 20, 25)]));
    let marks = frame.marks();
    assert_eq!(marks.len(), 1, "one prompt is in rows 8 to 17");
    let mark = marks[0];
    assert_eq!((mark.id, mark.row, mark.header), (BlockId(2), 2, 3));
    assert_eq!(mark.header_cols, 7);
    assert_eq!(mark.outcome, Outcome::Failed(ExitCode(1)));
}

#[test]
fn a_block_ends_on_the_row_before_its_end_mark_at_a_line_start() {
    let frame = Frame::of(read(0, 20, vec![done(1, 2, 8)]));
    assert_eq!(frame.last_row(BlockId(1)), Some(7));
    let mut mid = done(2, 10, 14);
    mid.end = End::At(at(14, 3));
    let frame = Frame::of(read(0, 20, vec![mid]));
    assert_eq!(
        frame.last_row(BlockId(2)),
        Some(14),
        "an end mid-line keeps its row"
    );
}

#[test]
fn a_running_block_runs_to_the_cursor_else_the_viewports_bottom() {
    let mut running = done(1, 2, 0);
    running.end = End::Running;
    running.outcome = Outcome::Running;
    let mut shown = read(0, 20, vec![running.clone()]);
    shown.cursor = Some(9);
    assert_eq!(Frame::of(shown).last_row(BlockId(1)), Some(9));
    let hidden = read(0, 20, vec![running]);
    assert_eq!(Frame::of(hidden).last_row(BlockId(1)), Some(19));
}

#[test]
fn the_output_starts_under_the_command_and_a_block_with_nothing_after_it_has_none() {
    let frame = Frame::of(read(5, 30, vec![done(1, 10, 20)]));
    // Command on row 11, output mark on row 12: rows 12 to 19, as the viewport shows them.
    assert_eq!(frame.output_rows(BlockId(1)), Some((7, 14)));
    let mut bare = done(2, 10, 12);
    bare.output = Some(at(12, 0));
    let frame = Frame::of(read(0, 30, vec![bare]));
    assert_eq!(
        frame.output_rows(BlockId(2)),
        None,
        "the command's row is the block's last"
    );
}

#[test]
fn a_block_heads_the_pane_while_its_prompt_is_above_it() {
    let frame = Frame::of(read(10, 20, vec![done(1, 2, 40)]));
    let header = frame.sticky_header().expect("block 1 holds the top row");
    assert_eq!((header.id, header.command.as_str()), (BlockId(1), "cmd1"));
    let near_end = Frame::of(read(39, 20, vec![done(1, 2, 40)]));
    assert_eq!(
        near_end.sticky_header(),
        None,
        "one row left is too few to head the pane"
    );
    let in_view = Frame::of(read(0, 20, vec![done(1, 2, 40)]));
    assert_eq!(in_view.sticky_header(), None, "its prompt is on screen");
}

#[test]
fn the_tools_sit_on_the_header_row_under_a_blank_lead() {
    let mut blank = done(1, 4, 12);
    blank.lead = Lead::Blank;
    let frame = Frame::of(read(0, 20, vec![blank]));
    let spot = frame.tools_spot(BlockId(1), 1).expect("on screen");
    assert_eq!(spot.row, 5, "the row under the blank lead");
    let plain = Frame::of(read(0, 20, vec![done(2, 4, 12)]));
    assert_eq!(plain.tools_spot(BlockId(2), 1).map(|s| s.row), Some(4));
}

#[test]
fn the_tools_go_to_the_top_row_when_the_prompt_scrolled_away_and_never_cover_the_cursor() {
    let mut r = read(10, 20, vec![done(1, 2, 40)]);
    assert_eq!(
        Frame::of(r.clone())
            .tools_spot(BlockId(1), 1)
            .map(|s| s.row),
        Some(0)
    );
    r.cursor = Some(10);
    assert_eq!(Frame::of(r).tools_spot(BlockId(1), 1), None);
}

#[test]
fn a_block_with_no_command_has_no_tools() {
    let mut empty = done(1, 4, 12);
    empty.command.clear();
    assert_eq!(
        Frame::of(read(0, 20, vec![empty])).tools_spot(BlockId(1), 1),
        None
    );
}

#[test]
fn a_row_belongs_to_the_newest_block_that_holds_it() {
    let mut running = done(2, 10, 0);
    running.end = End::Running;
    let frame = Frame::of(read(0, 30, vec![done(1, 2, 12), running]));
    assert_eq!(frame.block_at_viewport_row(5), Some(BlockId(1)));
    assert_eq!(
        frame.block_at_viewport_row(11),
        Some(BlockId(2)),
        "the newer block wins"
    );
    assert_eq!(
        frame.block_at_viewport_row(29),
        Some(BlockId(2)),
        "a running block has no end"
    );
    assert_eq!(frame.block_at_viewport_row(0), None);
}

#[test]
fn a_block_whose_end_the_terminal_lost_holds_no_rows() {
    let mut lost = done(1, 2, 0);
    lost.end = End::Lost;
    let frame = Frame::of(read(0, 30, vec![lost]));
    assert_eq!(frame.block_at_viewport_row(5), None);
    assert_eq!(frame.last_row(BlockId(1)), None);
}

#[test]
fn the_cards_follow_the_blocks_picks_and_the_live_prompt_in_row_order() {
    let mut first = done(1, 3, 8);
    first.picked = true;
    first.bookmarked = true;
    let mut r = read(2, 20, vec![first]);
    r.live = Some(LiveRead {
        prompt: at(9, 0),
        lead: Lead::Blank,
        input: Some(at(11, 2)),
    });
    let cards = Frame::of(r).cards(15);
    assert_eq!(cards.len(), 2);
    let (block, live) = (cards[0], cards[1]);
    assert_eq!(
        (block.first, block.pick, block.mark),
        (1, Pick::Selected, Mark::Bookmarked)
    );
    assert_eq!(block.stage, Stage::Ran(Outcome::Succeeded));
    assert_eq!(
        block.command,
        Some((2, 2, 2)),
        "the command's one row, from its column"
    );
    assert_eq!(
        (live.first, live.stage, live.lead),
        (7, Stage::AtPrompt, Lead::Blank)
    );
    assert_eq!(
        live.command,
        Some((9, 15, 2)),
        "the live command runs to `end`"
    );
}
