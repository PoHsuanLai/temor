//! Rows left out: where the slices go, which rows show and what a point is over.

use temor_vt::{CellPx, Px, Row, SurfacePoint};

use super::RowLayout;
use crate::metrics::fit_grid;
use crate::metrics::{CellMetrics, Fold, LayoutParams, Padding, Pin, PixelSize, Screen, Slice};

/// A 100 by 200 pane of 10 by 20 cells with 5 pixels of padding: 9 rows from y 5 to 185, 15
/// pixels of padding below; `folded` rows (first, last) left out.
fn layout(pin: Pin, screen: Screen, folded: &[(u16, u16)]) -> RowLayout {
    let metrics = CellMetrics {
        cell: CellPx::new(10, 20),
        baseline: Px(15),
        underline: Px(16),
        strikeout: Px(10),
        thickness: Px(1),
    };
    let fit = fit_grid(PixelSize::new(100, 200), &metrics, Padding::uniform(Px(5)));
    let folded: Vec<Fold> = folded
        .iter()
        .map(|&(first, last)| Fold {
            first: Row(first),
            last: Row(last),
        })
        .collect();
    RowLayout::new(LayoutParams {
        fit,
        folded: &folded,
        pin,
        screen,
    })
}

fn grid(y: i32) -> SurfacePoint {
    SurfacePoint { x: 7, y }
}

#[test]
fn with_nothing_left_out_the_grid_is_shown_as_drawn() {
    for (name, got) in [
        ("no folds", layout(Pin::Bottom, Screen::Primary, &[])),
        (
            "identity",
            RowLayout::identity(layout(Pin::Top, Screen::Primary, &[]).fit()),
        ),
    ] {
        assert!(!got.has_folds(), "{name}");
        assert_eq!(
            got.slices(),
            [Slice {
                src_y: Px(0),
                height: Px(200),
                dst_y: Px(0),
            }],
            "{name}"
        );
        assert_eq!(got.visible_rows(), 0..9, "{name}");
        assert_eq!(got.row_top(Row(0)), 5, "{name}");
        assert_eq!(got.row_top(Row(8)), 165, "{name}");
        for y in [-10, 100, 250] {
            assert_eq!(got.to_grid(grid(y)), grid(y), "{name}: y {y}");
        }
    }
}

#[test]
fn folded_rows_take_no_room_and_the_rows_below_them_move_up() {
    // Rows 2 and 3 are folded: the 9 rows are 5 + 20 * row. Bottom pin keeps the last row where
    // it was, so the rows above the fold move down by the two rows' height.
    let got = layout(Pin::Bottom, Screen::Primary, &[(2, 3)]);
    assert!(got.has_folds());
    assert_eq!(got.row_top(Row(8)), 5 + 8 * 20, "the last row stays");
    assert_eq!(
        got.row_top(Row(4)),
        5 + 4 * 20,
        "below the fold: stays, as the last row does"
    );
    assert_eq!(
        got.row_top(Row(1)),
        5 + 20 + 40,
        "above the fold moves down too"
    );
    assert_eq!(got.row_top(Row(0)), 5 + 40);
    // A folded row is shown where the fold begins.
    assert_eq!(got.row_top(Row(2)), got.row_top(Row(3)));
    assert_eq!(got.row_top(Row(2)), got.row_top(Row(4)));
    assert!(!got.visible_rows().is_empty());
    // Two slices (above and below the fold), none for the folded rows.
    let slices = got.slices();
    assert_eq!(slices.len(), 2, "{slices:?}");
    let shown: u32 = slices.iter().map(|s| s.height.0).sum();
    assert_eq!(shown, 200 - 40, "the whole picture less the folded rows");
}

#[test]
fn a_top_pin_leaves_the_rows_above_a_fold_in_place() {
    let got = layout(Pin::Top, Screen::Primary, &[(2, 3)]);
    assert_eq!(got.row_top(Row(0)), 5);
    assert_eq!(
        got.row_top(Row(2)),
        5 + 2 * 20,
        "the fold begins where its first row was"
    );
    assert_eq!(
        got.row_top(Row(4)),
        5 + 2 * 20,
        "the row after it takes its place"
    );
    assert_eq!(got.row_top(Row(8)), 5 + 6 * 20);
}

#[test]
fn a_point_is_over_the_rows_as_they_are_shown_and_never_a_folded_one() {
    let got = layout(Pin::Top, Screen::Primary, &[(2, 3)]);
    // Shown y 45 is the first pixel under the fold: it is row 4's first pixel, drawn at 85.
    assert_eq!(got.to_grid(grid(45)), grid(85));
    assert_eq!(got.to_grid(grid(44)), grid(44));
}

#[test]
fn the_alternate_screen_never_folds() {
    let got = layout(Pin::Bottom, Screen::Alternate, &[(2, 3)]);
    assert!(!got.has_folds());
}
