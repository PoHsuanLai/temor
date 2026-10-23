//! The row arithmetic of the sticky header, the tools and the command line, with no terminal.

use super::{Span, ViewRows, command_rows, heads_viewport, tools_row};

fn view(top: u32, bottom: u32, cursor: Option<u32>) -> ViewRows {
    ViewRows {
        top,
        bottom,
        cursor,
    }
}

#[test]
fn a_block_heads_the_viewport_only_when_its_prompt_is_above_and_two_rows_show() {
    // (block first..last, viewport top..bottom, heads)
    let table = [
        ((0, 50), (10, 33), true),
        // The prompt is on screen: nothing to show.
        ((10, 50), (10, 33), false),
        ((12, 50), (10, 33), false),
        // Two rows left (10 and 11): enough. One: not.
        ((0, 11), (10, 33), true),
        ((0, 10), (10, 33), false),
        // The block runs on past the viewport: the bottom clips it.
        ((0, 500), (10, 33), true),
        // The block ended above the viewport.
        ((0, 5), (10, 33), false),
    ];
    for ((first, last), (top, bottom), heads) in table {
        assert_eq!(
            heads_viewport(Span { first, last }, &view(top, bottom, None)),
            heads,
            "block {first}..{last} in viewport {top}..{bottom}"
        );
    }
}

#[test]
fn the_tools_sit_in_the_header_row_or_the_sticky_header_and_avoid_the_cursor() {
    // (block, viewport, reach, expected screen row)
    let table = [
        // The prompt is on screen: the tools sit on its row.
        ((12, 20), view(10, 33, Some(30)), 2, Some(12)),
        // The prompt scrolled away with enough rows left: the sticky header, the top row.
        ((2, 20), view(10, 33, Some(30)), 2, Some(10)),
        // The prompt scrolled away and one row left: no header, so no tools.
        ((2, 10), view(10, 33, Some(30)), 2, None),
        // The tools would cover the cursor's row.
        ((12, 20), view(10, 33, Some(13)), 2, None),
        ((12, 20), view(10, 33, Some(12)), 2, None),
        // Just clear of it.
        ((12, 20), view(10, 33, Some(14)), 2, Some(12)),
        ((12, 20), view(10, 33, Some(11)), 2, Some(12)),
        // No cursor shown.
        ((12, 20), view(10, 33, None), 2, Some(12)),
        // The block is below the viewport.
        ((40, 50), view(10, 33, None), 2, None),
    ];
    for ((first, last), viewport, reach, expected) in table {
        assert_eq!(
            tools_row(Span { first, last }, &viewport, reach),
            expected,
            "block {first}..{last} in {viewport:?}"
        );
    }
}

#[test]
fn a_command_ends_before_the_output_mark_at_a_line_start() {
    // (what, input row, output mark (row, col), end if no output yet, rows)
    for (what, input, output, end, rows) in [
        ("output on the next line", 4, Some((5, 0)), 9, (4, 4)),
        ("a wrapped command", 4, Some((6, 0)), 9, (4, 5)),
        ("output mark mid-line", 4, Some((4, 7)), 9, (4, 4)),
        ("still typing", 4, None, 6, (4, 6)),
        ("the cursor above the input mark", 4, None, 2, (4, 4)),
    ] {
        assert_eq!(command_rows(input, output, end), rows, "{what}");
    }
}
