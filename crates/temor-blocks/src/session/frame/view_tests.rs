//! The frame's answers from tables of plain reads: no terminal, no session.

use super::Frame;
use crate::block::Outcome;
use crate::session::frame::{BlockRead, End, Lead, Read, Spot};
use temor_core::vocab::ids::BlockId;

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
        prompt_lost: false,
        output: Some(at(first + 2, 0)),
        input: Some(at(first + 1, 2)),
        end: End::At(at(end, 0)),
        lead: Lead::Header,
        picked: false,
    }
}

fn read(top: u32, len: u32, blocks: Vec<BlockRead>) -> Read {
    Read {
        top,
        len,
        cursor: None,
        blocks,
        live: None,
    }
}

#[test]
fn marks_are_the_blocks_whose_prompts_are_in_the_viewport() {
    let mut blank = done(2, 10, 15);
    blank.lead = Lead::Blank;
    let frame = Frame::of(read(8, 10, vec![done(1, 2, 7), blank, done(3, 20, 25)]));
    let marks = frame.marks();
    assert_eq!(marks.len(), 1, "one prompt is in rows 8 to 17");
    let mark = marks[0];
    assert_eq!((mark.id, mark.row, mark.header), (BlockId(2), 2, 3));
}

#[test]
fn a_block_ends_on_the_row_before_its_end_mark_at_a_line_start() {
    let frame = Frame::of(read(0, 20, vec![done(1, 2, 8)]));
    assert_eq!(frame.output_rows(BlockId(1)), Some((4, 7)));
    let mut mid = done(2, 10, 14);
    mid.end = End::At(at(14, 3));
    let frame = Frame::of(read(0, 20, vec![mid]));
    assert_eq!(
        frame.output_rows(BlockId(2)),
        Some((12, 14)),
        "an end mid-line keeps its row"
    );
}

#[test]
fn a_running_block_runs_to_the_cursor_else_the_viewports_bottom() {
    let mut running = done(1, 2, 0);
    running.end = End::Running;
    let mut shown = read(0, 20, vec![running.clone()]);
    shown.cursor = Some(9);
    assert_eq!(Frame::of(shown).output_rows(BlockId(1)), Some((4, 9)));
    let hidden = read(0, 20, vec![running]);
    assert_eq!(Frame::of(hidden).output_rows(BlockId(1)), Some((4, 19)));
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
fn a_row_belongs_to_the_newest_block_that_holds_it() {
    let mut running = done(2, 10, 0);
    running.end = End::Running;
    let frame = Frame::of(read(0, 30, vec![done(1, 2, 12), running]));
    assert_eq!(frame.block_at_screen_row(5), Some(BlockId(1)));
    assert_eq!(
        frame.block_at_screen_row(11),
        Some(BlockId(2)),
        "the newer block wins"
    );
    assert_eq!(
        frame.block_at_screen_row(29),
        Some(BlockId(2)),
        "a running block has no end"
    );
    assert_eq!(frame.block_at_screen_row(0), None);
}

#[test]
fn a_block_whose_end_the_terminal_lost_holds_no_rows() {
    let mut lost = done(1, 2, 0);
    lost.end = End::Lost;
    let frame = Frame::of(read(0, 30, vec![lost]));
    assert_eq!(frame.block_at_screen_row(5), None);
    assert_eq!(frame.output_rows(BlockId(1)), None);
    assert!(
        frame.pick_order().is_empty(),
        "a lost block can't be picked"
    );
}

#[test]
fn a_block_whose_prompt_the_scrollback_took_is_gone_though_its_output_mark_is_kept() {
    let mut trimmed = done(1, 0, 6);
    trimmed.prompt = None;
    trimmed.prompt_lost = true;
    let frame = Frame::of(read(0, 20, vec![trimmed, done(2, 8, 12)]));
    assert_eq!(frame.block_at_screen_row(3), None);
    assert_eq!(frame.pick_order(), vec![BlockId(2)]);
    assert_eq!(frame.output_rows(BlockId(1)), None);
    assert_eq!(frame.block_first_row(BlockId(1)), None);
}

#[test]
fn a_block_starts_on_its_prompts_row_or_its_outputs_without_one() {
    let mut bare = done(2, 10, 14);
    bare.prompt = None;
    let mut lost = done(3, 20, 25);
    lost.end = End::Lost;
    let frame = Frame::of(read(0, 30, vec![done(1, 2, 8), bare, lost]));
    assert_eq!(frame.block_first_row(BlockId(1)), Some(2));
    assert_eq!(frame.block_first_row(BlockId(2)), Some(12));
    assert_eq!(frame.block_first_row(BlockId(3)), None, "its end is gone");
    assert_eq!(frame.block_first_row(BlockId(9)), None);
}
