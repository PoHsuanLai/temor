use temor_vt::{Col, Rgb, Row};

use super::{Alpha, Cut, RowBand, RowSpan, RowWeight, Tint, look_of};

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
        top: Cut::Edge,
        bottom: Cut::Edge,
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
        assert_eq!(
            look.upper.fill.map(|t| t.color.r),
            fill,
            "fill of row {row}"
        );
        assert_eq!(look.upper, look.lower, "row {row} is one piece");
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

#[test]
fn a_band_cut_across_middles_shares_its_end_rows_with_its_neighbours() {
    // Two cards sharing row 4: the first ends across its middle, the second begins there.
    let mut first = band(1, 4, tint(1, 10), tint(9, 20));
    (first.top, first.bottom) = (Cut::Middle, Cut::Middle);
    let mut second = band(4, 7, tint(2, 30), tint(9, 20));
    second.top = Cut::Middle;
    let bands = [first, second];
    // (row, upper fill red, lower fill red, rule)
    for (row, upper, lower, rule) in [
        (0, None, None, None),
        (1, None, Some(1), Some(Cut::Middle)),
        (2, Some(1), Some(1), None),
        (4, Some(1), Some(2), Some(Cut::Middle)),
        (7, Some(2), Some(2), None),
    ] {
        let look = look_of(&bands, Row(row));
        assert_eq!(
            look.upper.fill.map(|t| t.color.r),
            upper,
            "upper of row {row}"
        );
        assert_eq!(
            look.lower.fill.map(|t| t.color.r),
            lower,
            "lower of row {row}"
        );
        assert_eq!(look.rule.map(|(_, cut)| cut), rule, "rule of row {row}");
    }
}

#[test]
fn bold_from_a_column_holds_on_the_first_row_only() {
    let mut band = band(3, 4, None, None);
    band.weight = RowWeight::BoldFrom(Col(4));
    let bands = [band];
    assert_eq!(look_of(&bands, Row(3)).weight, RowWeight::BoldFrom(Col(4)));
    assert_eq!(look_of(&bands, Row(4)).weight, RowWeight::Bold);
    let first = look_of(&bands, Row(3)).weight;
    assert!(!first.bolds(Col(3)) && first.bolds(Col(4)));
}
