use temor_vt::Rgb;

use super::super::diacritics::mark;
use super::*;

/// A placeholder cell's text: the character, then the given marks (by number).
fn text(marks: &[usize]) -> String {
    std::iter::once(PLACEHOLDER)
        .chain(marks.iter().map(|&m| mark(m)))
        .collect()
}

fn cell(col: u16, marks: &[usize], fg: ColorRef) -> PlaceholderCell {
    PlaceholderCell::decode(Col(col), &text(marks), fg, ColorRef::Unset).expect("a placeholder")
}

const RED: ColorRef = ColorRef::Rgb(Rgb::new(0, 0, 7));

#[test]
fn a_cell_decodes_by_its_marks_and_colours() {
    struct Case {
        name: &'static str,
        text: String,
        fg: ColorRef,
        underline: ColorRef,
        want: Option<(Option<u32>, Option<u32>, Option<u8>, u32, u32)>,
    }
    let plain = |name, text, want| Case {
        name,
        text,
        fg: RED,
        underline: ColorRef::Unset,
        want,
    };
    let cases = [
        plain("not a placeholder", "a".to_owned(), None),
        plain("blank", String::new(), None),
        plain("no marks", text(&[]), Some((None, None, None, 7, 0))),
        plain("row only", text(&[3]), Some((Some(3), None, None, 7, 0))),
        plain(
            "row and column",
            text(&[2, 5]),
            Some((Some(2), Some(5), None, 7, 0)),
        ),
        plain(
            "high byte",
            text(&[0, 1, 4]),
            Some((Some(0), Some(1), Some(4), 7, 0)),
        ),
        plain(
            "a fourth mark is ignored",
            text(&[0, 1, 4, 9]),
            Some((Some(0), Some(1), Some(4), 7, 0)),
        ),
        plain(
            "a high byte past 255 is absent",
            text(&[0, 1, 256]),
            Some((Some(0), Some(1), None, 7, 0)),
        ),
        plain(
            "a plain accent is not a number",
            format!("{PLACEHOLDER}\u{0301}{}", mark(1)),
            Some((None, Some(1), None, 7, 0)),
        ),
        Case {
            name: "palette colour",
            text: text(&[]),
            fg: ColorRef::Palette(200),
            underline: ColorRef::Unset,
            want: Some((None, None, None, 200, 0)),
        },
        Case {
            name: "colour packs 24 bits",
            text: text(&[]),
            fg: ColorRef::Rgb(Rgb::new(1, 2, 3)),
            underline: ColorRef::Unset,
            want: Some((None, None, None, 0x01_02_03, 0)),
        },
        Case {
            name: "underline colour is the placement",
            text: text(&[]),
            fg: RED,
            underline: ColorRef::Palette(9),
            want: Some((None, None, None, 7, 9)),
        },
    ];
    for c in cases {
        let got = PlaceholderCell::decode(Col(0), &c.text, c.fg, c.underline)
            .map(|p| (p.row, p.column, p.image_high, p.image_low, p.placement));
        assert_eq!(got, c.want, "{}", c.name);
    }
}

fn run(col: u16, width: u32, image: u32, row: u32, column: u32) -> PlaceholderRun {
    PlaceholderRun {
        col: Col(col),
        width,
        image: ImageId(image),
        placement: PlacementId(0),
        row,
        column,
    }
}

#[test]
fn neighbouring_cells_join_into_runs_as_kitty_does() {
    let other = ColorRef::Rgb(Rgb::new(0, 0, 8));
    struct Case {
        name: &'static str,
        cells: Vec<PlaceholderCell>,
        want: Vec<PlaceholderRun>,
    }
    let cases = [
        Case {
            name: "a lone cell with no marks is the image's top-left",
            cells: vec![cell(4, &[], RED)],
            want: vec![run(4, 1, 7, 0, 0)],
        },
        Case {
            name: "later cells may leave their marks out",
            cells: vec![cell(0, &[1, 2], RED), cell(1, &[], RED), cell(2, &[], RED)],
            want: vec![run(0, 3, 7, 1, 2)],
        },
        Case {
            name: "marks that count up continue the run",
            cells: vec![cell(0, &[0, 0], RED), cell(1, &[0, 1], RED)],
            want: vec![run(0, 2, 7, 0, 0)],
        },
        Case {
            name: "a column that skips starts a new run",
            cells: vec![cell(0, &[0, 0], RED), cell(1, &[0, 3], RED)],
            want: vec![run(0, 1, 7, 0, 0), run(1, 1, 7, 0, 3)],
        },
        Case {
            name: "a different row starts a new run",
            cells: vec![cell(0, &[0, 0], RED), cell(1, &[1, 1], RED)],
            want: vec![run(0, 1, 7, 0, 0), run(1, 1, 7, 1, 1)],
        },
        Case {
            name: "a different image starts a new run",
            cells: vec![cell(0, &[], RED), cell(1, &[], other)],
            want: vec![run(0, 1, 7, 0, 0), run(1, 1, 8, 0, 0)],
        },
        Case {
            name: "a gap in the row breaks a run even if the marks agree",
            cells: vec![cell(0, &[0, 0], RED), cell(2, &[0, 1], RED)],
            want: vec![run(0, 1, 7, 0, 0), run(2, 1, 7, 0, 1)],
        },
        Case {
            name: "the high byte is the top of the id",
            cells: vec![cell(0, &[0, 0, 2], RED), cell(1, &[], RED)],
            want: vec![run(0, 2, 7 | (2 << 24), 0, 0)],
        },
        Case {
            name: "a high byte the run lacks breaks it",
            cells: vec![cell(0, &[0, 0], RED), cell(1, &[0, 1, 2], RED)],
            want: vec![run(0, 1, 7, 0, 0), run(1, 1, 7 | (2 << 24), 0, 1)],
        },
        Case {
            name: "no cells, no runs",
            cells: vec![],
            want: vec![],
        },
    ];
    for c in cases {
        assert_eq!(runs(&c.cells), c.want, "{}", c.name);
    }
}
