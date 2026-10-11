//! Unit tests for the mark helpers that need no terminal.

use temor_core::block::Outcome;
use temor_core::vocab::ids::ExitCode;
use temor_render::{Palette, RowBand, RowSpan, RowWeight, Tint};
use temor_vt::{Rgb, Row};

use super::actions::output_name;
use super::bands::{CardBlock, Pick, Stage, bands_of, command_rows};
use crate::view::card_colours::CardColours;

#[test]
fn output_names_come_from_the_command_or_a_fallback() {
    assert_eq!(output_name("make all"), "make all output.txt");
    assert_eq!(output_name("  "), "Command output.txt");
    assert_eq!(output_name(&"x".repeat(60)).len(), 40 + " output.txt".len());
}

fn block(first: i64, stage: Stage, pick: Pick) -> CardBlock {
    CardBlock {
        first,
        stage,
        pick,
        command: None,
    }
}

fn band(first: u16, last: u16, parts: [Option<Tint>; 3]) -> RowBand {
    let [fill, stripe, rule] = parts;
    RowBand {
        rows: RowSpan {
            first: Row(first),
            last: Row(last),
        },
        fill,
        stripe,
        rule,
        weight: RowWeight::Normal,
    }
}

#[test]
fn blocks_become_bands_by_how_they_stand() {
    let c = CardColours::of(&Palette::default(), Rgb::new(10, 20, 30));
    let failed = Stage::Ran(Outcome::Failed(ExitCode(1)));
    let done = Stage::Ran(Outcome::Succeeded);
    let stopped = Stage::Ran(Outcome::Stopped(ExitCode(130)));
    let (plain, chosen) = (Pick::Unselected, Pick::Selected);
    // (what, blocks, cursor row, viewport rows, bands)
    let cases = [
        (
            "a failed block gets fill and stripe, no rule on row 0; the next only a rule",
            vec![block(0, failed, plain), block(3, done, plain)],
            5,
            10,
            vec![
                band(0, 2, [Some(c.failed_fill), Some(c.failed_stripe), None]),
                band(3, 5, [None, None, Some(c.rule)]),
            ],
        ),
        (
            "selected wins the fill over failed, which keeps its stripe",
            vec![block(2, failed, chosen)],
            4,
            10,
            vec![band(
                2,
                4,
                [Some(c.selected_fill), Some(c.failed_stripe), Some(c.rule)],
            )],
        ),
        (
            "the newest block at its prompt is set off by its rule alone, with no fill",
            vec![block(1, done, plain), block(4, Stage::AtPrompt, plain)],
            5,
            10,
            vec![
                band(1, 3, [None, None, Some(c.rule)]),
                band(4, 5, [None, None, Some(c.rule)]),
            ],
        ),
        (
            "the selected fill shows on the block at its prompt",
            vec![block(3, Stage::AtPrompt, chosen)],
            4,
            10,
            vec![band(3, 4, [Some(c.selected_fill), None, Some(c.rule)])],
        ),
        (
            "a stopped block is quiet",
            vec![block(2, stopped, plain)],
            3,
            10,
            vec![band(2, 3, [None, None, Some(c.rule)])],
        ),
        (
            "a block begun above the view is clipped and shows no rule",
            vec![block(-4, failed, plain), block(2, done, plain)],
            3,
            10,
            vec![
                band(0, 1, [Some(c.failed_fill), Some(c.failed_stripe), None]),
                band(2, 3, [None, None, Some(c.rule)]),
            ],
        ),
        (
            "a block past the bottom is clipped to the view",
            vec![block(8, failed, plain)],
            20,
            10,
            vec![band(
                8,
                9,
                [Some(c.failed_fill), Some(c.failed_stripe), Some(c.rule)],
            )],
        ),
        ("no blocks, no bands", vec![], 3, 10, vec![]),
    ];
    for (what, blocks, end, rows, expected) in cases {
        assert_eq!(bands_of(&blocks, end, rows, &c), expected, "{what}");
    }
}

#[test]
fn the_command_line_is_a_bold_band_clipped_to_the_view() {
    let c = CardColours::of(&Palette::default(), Rgb::new(10, 20, 30));
    let done = Stage::Ran(Outcome::Succeeded);
    let bold = |first: u16, last: u16| RowBand {
        rows: RowSpan {
            first: Row(first),
            last: Row(last),
        },
        fill: None,
        stripe: None,
        rule: None,
        weight: RowWeight::Bold,
    };
    let with = |first, command| CardBlock {
        command: Some(command),
        ..block(first, done, Pick::Unselected)
    };
    // (what, block, expected bold band)
    for (what, card, expected) in [
        ("the typing line", with(2, (3, 3)), Some(bold(3, 3))),
        ("a wrapped command", with(2, (3, 4)), Some(bold(3, 4))),
        (
            "scrolled half out of the top",
            with(-3, (-1, 1)),
            Some(bold(0, 1)),
        ),
        ("scrolled out of the top", with(-3, (-2, -1)), None),
        ("past the bottom", with(7, (9, 12)), Some(bold(9, 9))),
        (
            "a block with no input mark",
            block(2, done, Pick::Unselected),
            None,
        ),
    ] {
        let got: Vec<RowBand> = bands_of(&[card], 9, 10, &c)
            .into_iter()
            .filter(|b| b.weight == RowWeight::Bold)
            .collect();
        assert_eq!(got, expected.into_iter().collect::<Vec<_>>(), "{what}");
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
