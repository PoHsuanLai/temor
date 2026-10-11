use temor_vt::{Rgb, Row};

use super::{Alpha, RowBand, RowSpan, RowWeight, Tint, look_of};

fn tint(r: u8, a: u8) -> Option<Tint> {
    Some(Tint {
        color: Rgb::new(r, 0, 0),
        alpha: Alpha(a),
    })
}

fn band(first: u16, last: u16, fill: Option<Tint>, rule: Option<Tint>) -> RowBand {
    RowBand {
        rows: RowSpan {
            first: Row(first),
            last: Row(last),
        },
        fill,
        stripe: None,
        rule,
        weight: RowWeight::Normal,
    }
}

#[test]
fn a_rule_sits_on_the_first_row_only_and_later_bands_win() {
    let bands = [
        band(2, 4, tint(1, 10), tint(9, 20)),
        band(4, 5, tint(2, 30), None),
    ];
    // (row, fill red, rule present)
    for (row, fill, rule) in [
        (1, None, false),
        (2, Some(1), true),
        (3, Some(1), false),
        (4, Some(2), false),
        (5, Some(2), false),
        (6, None, false),
    ] {
        let look = look_of(&bands, Row(row));
        assert_eq!(look.fill.map(|t| t.color.r), fill, "fill of row {row}");
        assert_eq!(look.rule.is_some(), rule, "rule of row {row}");
    }
}

#[test]
fn a_bold_band_makes_its_rows_bold_and_no_other() {
    let mut bold = band(3, 4, None, None);
    bold.weight = RowWeight::Bold;
    let bands = [band(2, 5, tint(1, 10), None), bold];
    // (row, weight)
    for (row, weight) in [
        (2, RowWeight::Normal),
        (3, RowWeight::Bold),
        (4, RowWeight::Bold),
        (5, RowWeight::Normal),
    ] {
        assert_eq!(look_of(&bands, Row(row)).weight, weight, "row {row}");
    }
}
