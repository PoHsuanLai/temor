//! Unit tests for the mark helpers that need no terminal.

use temor_core::block::Outcome;
use temor_core::vocab::ids::ExitCode;
use temor_render::{Palette, RowBand, RowSpan, Tint};
use temor_vt::{Rgb, Row};

use super::actions::output_name;
use super::bands::{CardBlock, Pick, Stage, bands_of};
use crate::view::card_colours::CardColours;

#[test]
fn output_names_come_from_the_command_or_a_fallback() {
    assert_eq!(output_name("make all"), "make all output.txt");
    assert_eq!(output_name("  "), "Command output.txt");
    assert_eq!(output_name(&"x".repeat(60)).len(), 40 + " output.txt".len());
}

fn block(first: i64, stage: Stage, pick: Pick) -> CardBlock {
    CardBlock { first, stage, pick }
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
            "the newest block at its prompt gets the input fill and keeps its rule",
            vec![block(1, done, plain), block(4, Stage::AtPrompt, plain)],
            5,
            10,
            vec![
                band(1, 3, [None, None, Some(c.rule)]),
                band(4, 5, [Some(c.input), None, Some(c.rule)]),
            ],
        ),
        (
            "once it runs the input fill is gone",
            vec![block(4, done, plain)],
            7,
            10,
            vec![band(4, 7, [None, None, Some(c.rule)])],
        ),
        (
            "selected and failed both win over the input fill",
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
