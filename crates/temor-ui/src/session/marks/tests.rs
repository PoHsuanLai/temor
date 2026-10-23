//! Unit tests for the mark helpers that need no terminal.

use temor_core::block::Outcome;
use temor_core::vocab::ids::ExitCode;
use temor_pane::text::{RowText, Wrap};
use temor_render::{Cut, Palette, RowBand, RowSpan, RowWeight, Tint};
use temor_vt::{Col, Row};

use super::actions::output_name;
use super::bands::{bands_of, leads_blank};
use crate::view::card_colours::CardColours;
use temor_core::session::frame::{CardBlock, Lead, Mark, Pick, Stage};

#[test]
fn output_names_come_from_the_command_or_a_fallback() {
    assert_eq!(output_name("make all"), "make all output.txt");
    assert_eq!(output_name("  "), "Command output.txt");
    assert_eq!(output_name(&"x".repeat(60)).len(), 40 + " output.txt".len());
}

fn block(first: i64, stage: Stage, pick: Pick) -> CardBlock {
    CardBlock {
        first,
        lead: Lead::Header,
        stage,
        pick,
        mark: Mark::Plain,
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
        top: Cut::Edge,
        bottom: Cut::Edge,
        fill,
        stripe,
        rule,
        weight: RowWeight::Normal,
    }
}

#[test]
fn blocks_become_bands_by_how_they_stand() {
    let c = CardColours::of(&Palette::default());
    let failed = Stage::Ran(Outcome::Failed(ExitCode(1)));
    let done = Stage::Ran(Outcome::Succeeded);
    let stopped = Stage::Ran(Outcome::Stopped(ExitCode(130)));
    let (plain, chosen) = (Pick::Unselected, Pick::Selected);
    // (what, blocks, cursor row, viewport rows, bands)
    let cases = [
        (
            "a failed block gets fill and stripe; a plain one draws nothing",
            vec![block(0, failed, plain), block(3, done, plain)],
            5,
            10,
            vec![band(
                0,
                2,
                [Some(c.failed_fill), Some(c.failed_stripe), None],
            )],
        ),
        (
            "selected wins the fill over failed, which keeps its stripe",
            vec![block(2, failed, chosen)],
            4,
            10,
            vec![band(
                2,
                4,
                [Some(c.selected_fill), Some(c.failed_stripe), None],
            )],
        ),
        (
            "a bookmark's stripe wins over a failure's",
            vec![CardBlock {
                mark: Mark::Bookmarked,
                ..block(2, failed, plain)
            }],
            4,
            10,
            vec![band(
                2,
                4,
                [Some(c.failed_fill), Some(c.bookmark_stripe), None],
            )],
        ),
        (
            "plain blocks and the one at its prompt draw nothing: their cards set them apart",
            vec![block(1, done, plain), block(4, Stage::AtPrompt, plain)],
            5,
            10,
            vec![],
        ),
        (
            "the selected fill shows on the block at its prompt",
            vec![block(3, Stage::AtPrompt, chosen)],
            4,
            10,
            vec![band(3, 4, [Some(c.selected_fill), None, None])],
        ),
        (
            "a stopped block is quiet",
            vec![block(2, stopped, plain)],
            3,
            10,
            vec![],
        ),
        (
            "a block begun above the view is clipped",
            vec![block(-4, failed, plain), block(2, done, plain)],
            3,
            10,
            vec![band(
                0,
                1,
                [Some(c.failed_fill), Some(c.failed_stripe), None],
            )],
        ),
        (
            "a block past the bottom is clipped to the view",
            vec![block(8, failed, plain)],
            20,
            10,
            vec![band(
                8,
                9,
                [Some(c.failed_fill), Some(c.failed_stripe), None],
            )],
        ),
        ("no blocks, no bands", vec![], 3, 10, vec![]),
    ];
    for (what, blocks, end, rows, expected) in cases {
        assert_eq!(bands_of(&blocks, end, rows, &c), expected, "{what}");
    }
}

#[test]
fn a_card_on_a_blank_line_begins_and_ends_across_that_lines_middle() {
    let c = CardColours::of(&Palette::default());
    let done = Stage::Ran(Outcome::Succeeded);
    let failed = Stage::Ran(Outcome::Failed(ExitCode(1)));
    let blank = |first, stage| CardBlock {
        lead: Lead::Blank,
        ..block(first, stage, Pick::Unselected)
    };
    let cut = |b: RowBand, top, bottom| RowBand { top, bottom, ..b };
    let red = [Some(c.failed_fill), Some(c.failed_stripe), None];
    let (edge, mid) = (Cut::Edge, Cut::Middle);
    // (what, blocks, cursor row, viewport rows, bands)
    let cases = [
        (
            "a failed card shares the blank lines above and below it with its neighbours",
            vec![
                block(0, done, Pick::Unselected),
                blank(3, failed),
                blank(6, done),
            ],
            8,
            10,
            vec![cut(band(3, 6, red), mid, mid)],
        ),
        (
            "a card scrolled above the view is clipped at the edge",
            vec![blank(-2, failed), blank(3, done)],
            5,
            10,
            vec![cut(
                band(0, 3, [Some(c.failed_fill), Some(c.failed_stripe), None]),
                edge,
                mid,
            )],
        ),
        (
            "the next blank line below the view is clipped at the edge",
            vec![blank(2, failed), blank(10, done)],
            12,
            10,
            vec![cut(band(2, 9, red), mid, edge)],
        ),
    ];
    for (what, blocks, end, rows, expected) in cases {
        assert_eq!(bands_of(&blocks, end, rows, &c), expected, "{what}");
    }
}

#[test]
fn a_prompt_leads_with_a_blank_line_only_when_its_header_follows() {
    // (what, first row's text, input mark's row, lead); the prompt's first row is 4.
    for (what, text, input, lead) in [
        ("temor's prompt", "      ", Some(6), Lead::Blank),
        ("a one-line prompt", "~ $ ", Some(4), Lead::Header),
        ("a prompt with text", "~/temor main", Some(5), Lead::Header),
        ("blank, not marked yet", "", None, Lead::Header),
        ("blank, typed on that row", "", Some(4), Lead::Header),
    ] {
        let row = RowText::plain(text, Wrap::Ends);
        assert_eq!(leads_blank(&row, input, 4), lead, "{what}");
    }
}

#[test]
fn the_command_line_is_a_bold_band_clipped_to_the_view() {
    let c = CardColours::of(&Palette::default());
    let done = Stage::Ran(Outcome::Succeeded);
    let bold = |first: u16, last: u16| RowBand {
        rows: RowSpan {
            first: Row(first),
            last: Row(last),
        },
        top: Cut::Edge,
        bottom: Cut::Edge,
        fill: None,
        stripe: None,
        rule: None,
        weight: RowWeight::Bold,
    };
    let with = |first, command: (i64, i64)| CardBlock {
        command: Some((command.0, command.1, 0)),
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
fn a_command_after_a_one_line_prompt_is_bold_from_its_column() {
    let c = CardColours::of(&Palette::default());
    let done = Stage::Ran(Outcome::Succeeded);
    let card = |first: i64, command| CardBlock {
        command: Some(command),
        ..block(first, done, Pick::Unselected)
    };
    let weights = |card: CardBlock| -> Vec<RowWeight> {
        bands_of(&[card], 9, 10, &c)
            .into_iter()
            .map(|b| b.weight)
            .filter(|w| *w != RowWeight::Normal)
            .collect()
    };
    // (what, block, the bold band's weight)
    for (what, block, want) in [
        (
            "after `~/temor $ `",
            card(2, (2, 2, 10)),
            RowWeight::BoldFrom(Col(10)),
        ),
        (
            "on a typing line of its own",
            card(2, (3, 3, 0)),
            RowWeight::Bold,
        ),
        (
            "its first row above the view",
            card(-3, (-1, 1, 10)),
            RowWeight::Bold,
        ),
    ] {
        assert_eq!(weights(block), vec![want], "{what}");
    }
}
