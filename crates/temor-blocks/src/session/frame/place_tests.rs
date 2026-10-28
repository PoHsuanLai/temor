//! The row arithmetic of the command line, with no terminal.

use super::command_rows;

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
