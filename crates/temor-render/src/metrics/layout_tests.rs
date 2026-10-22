//! Room between rows: where the slices and gaps go, which rows show and what a point is over.

use temor_vt::{CellPx, Px, Row, SurfacePoint};

use super::RowLayout;
use crate::metrics::{CellMetrics, Padding, PixelSize, fit_grid};
use crate::metrics::{Gap, GridHit, Insert, LayoutParams, Pin, Screen, Slice};

/// A 100 by 200 pane of 10 by 20 cells with 5 pixels of padding: 9 rows from y 5 to 185, 15
/// pixels of padding below.
fn layout(lower: u16, pin: Pin, screen: Screen, inserts: &[(u16, u32)]) -> RowLayout {
    let metrics = CellMetrics {
        cell: CellPx::new(10, 20),
        baseline: Px(15),
        underline: Px(16),
        strikeout: Px(10),
        thickness: Px(1),
    };
    let fit = fit_grid(PixelSize::new(100, 200), &metrics, Padding::uniform(Px(5))).lowered(lower);
    let inserts: Vec<Insert> = inserts
        .iter()
        .map(|&(after, height)| Insert {
            after: Row(after),
            height: Px(height),
        })
        .collect();
    RowLayout::new(LayoutParams {
        fit,
        lower,
        inserts: &inserts,
        folded: &[],
        pin,
        screen,
    })
}

fn slice(src_y: u32, height: u32, dst_y: u32) -> Slice {
    Slice {
        src_y: Px(src_y),
        height: Px(height),
        dst_y: Px(dst_y),
    }
}

fn gap(after: u16, dst_y: u32, height: u32) -> Gap {
    Gap {
        after: Row(after),
        dst_y: Px(dst_y),
        height: Px(height),
    }
}

fn grid(y: i32) -> GridHit {
    GridHit::Grid(SurfacePoint { x: 7, y })
}

#[test]
fn rows_are_shown_around_the_room_made_under_them() {
    use Pin::{Bottom, Top};
    use Screen::{Alternate, Primary};
    struct Case {
        name: &'static str,
        lower: u16,
        pin: Pin,
        screen: Screen,
        inserts: &'static [(u16, u32)],
        slices: Vec<Slice>,
        gaps: Vec<Gap>,
        visible: std::ops::Range<u16>,
        /// (row, shown top)
        tops: &'static [(u16, i32)],
        /// (pane y, what it is over)
        hits: Vec<(i32, GridHit)>,
    }
    let identity = |name, inserts, screen| Case {
        name,
        lower: 0,
        pin: Bottom,
        screen,
        inserts,
        slices: vec![slice(0, 200, 0)],
        gaps: vec![],
        visible: 0..9,
        tops: &[(0, 5), (8, 165)],
        hits: vec![(-10, grid(-10)), (100, grid(100)), (250, grid(250))],
    };
    let cases = [
        identity("no room: shown as drawn", &[], Primary),
        identity(
            "room under rows the grid lacks is left out",
            &[(9, 40)],
            Primary,
        ),
        identity(
            "a full-screen program's screen has no room",
            &[(4, 40)],
            Alternate,
        ),
        Case {
            name: "bottom pin: the rows above move up and the top ones are cut",
            lower: 0,
            pin: Bottom,
            screen: Primary,
            inserts: &[(4, 40)],
            slices: vec![slice(40, 65, 0), slice(105, 95, 105)],
            gaps: vec![gap(4, 65, 40)],
            visible: 2..9,
            tops: &[(0, -35), (2, 5), (4, 45), (5, 105), (8, 165)],
            hits: vec![
                (10, grid(50)),
                (64, grid(104)),
                (65, GridHit::Gap { after: Row(4) }),
                (104, GridHit::Gap { after: Row(4) }),
                (105, grid(105)),
                (-5, grid(35)),
            ],
        },
        Case {
            name: "top pin: the rows below move down and the bottom ones are cut",
            lower: 0,
            pin: Top,
            screen: Primary,
            inserts: &[(4, 40)],
            slices: vec![slice(0, 105, 0), slice(105, 55, 145)],
            gaps: vec![gap(4, 105, 40)],
            visible: 0..7,
            tops: &[(0, 5), (4, 85), (5, 145)],
            hits: vec![
                (50, grid(50)),
                (120, GridHit::Gap { after: Row(4) }),
                (150, grid(110)),
                (300, grid(260)),
            ],
        },
        Case {
            name: "the blank rows the grid was lowered over are taken first",
            lower: 3,
            pin: Top,
            screen: Primary,
            inserts: &[(4, 40)],
            slices: vec![slice(40, 125, 0), slice(165, 35, 165)],
            gaps: vec![gap(4, 125, 40)],
            visible: 0..6,
            tops: &[(0, 25), (4, 105), (5, 165)],
            hits: vec![(30, grid(70)), (130, GridHit::Gap { after: Row(4) })],
        },
        Case {
            name: "top pin with more room than blank rows: the rest pushes down",
            lower: 1,
            pin: Top,
            screen: Primary,
            inserts: &[(4, 40)],
            slices: vec![slice(20, 105, 0), slice(125, 55, 145)],
            gaps: vec![gap(4, 105, 40)],
            visible: 0..7,
            tops: &[(0, 5), (5, 145)],
            hits: vec![(170, grid(150))],
        },
        Case {
            name: "two inserts under one row add up, and one under the last row",
            lower: 0,
            pin: Bottom,
            screen: Primary,
            inserts: &[(2, 20), (2, 20), (8, 20)],
            slices: vec![slice(60, 5, 0), slice(65, 135, 45)],
            gaps: vec![gap(2, 5, 40), gap(8, 180, 20)],
            visible: 3..9,
            tops: &[(2, -15), (3, 45), (8, 145)],
            hits: vec![
                (0, grid(60)),
                (10, GridHit::Gap { after: Row(2) }),
                (190, GridHit::Gap { after: Row(8) }),
                (400, GridHit::Gap { after: Row(8) }),
            ],
        },
    ];
    for case in cases {
        let got = layout(case.lower, case.pin, case.screen, case.inserts);
        let name = case.name;
        assert_eq!(got.slices(), case.slices.as_slice(), "{name}: slices");
        assert_eq!(got.gaps(), case.gaps.as_slice(), "{name}: gaps");
        assert_eq!(got.visible_rows(), case.visible, "{name}: visible rows");
        for &(row, top) in case.tops {
            assert_eq!(got.row_top(Row(row)), top, "{name}: top of row {row}");
        }
        for (y, want) in case.hits {
            assert_eq!(got.to_grid(SurfacePoint { x: 7, y }), want, "{name}: y {y}");
        }
        for g in got.gaps() {
            assert_eq!(
                got.insert_top(g.after),
                Some(g.dst_y.0 as i32),
                "{name}: the room's top"
            );
        }
        // The shown parts cover the pane once, top to bottom.
        let mut parts: Vec<(u32, u32)> = got
            .slices()
            .iter()
            .map(|s| (s.dst_y.0, s.height.0))
            .chain(got.gaps().iter().map(|g| (g.dst_y.0, g.height.0)))
            .collect();
        parts.sort_unstable();
        let mut y = 0;
        for (at, height) in parts {
            assert_eq!(at, y, "{name}: no hole or overlap at {y}");
            y += height;
        }
        assert_eq!(y, 200, "{name}: the whole pane");
    }
}

#[test]
fn no_room_under_a_row_without_an_insert() {
    let got = layout(0, Pin::Bottom, Screen::Primary, &[(4, 40)]);
    assert_eq!(got.insert_top(Row(3)), None);
    assert_eq!(RowLayout::identity(got.fit()).insert_top(Row(4)), None);
}

/// The same pane as [`layout`], with `folded` rows (first, last) left out.
fn folded_layout(pin: Pin, folded: &[(u16, u16)], inserts: &[(u16, u32)]) -> RowLayout {
    let metrics = CellMetrics {
        cell: CellPx::new(10, 20),
        baseline: Px(15),
        underline: Px(16),
        strikeout: Px(10),
        thickness: Px(1),
    };
    let fit = fit_grid(PixelSize::new(100, 200), &metrics, Padding::uniform(Px(5)));
    let inserts: Vec<Insert> = inserts
        .iter()
        .map(|&(after, height)| Insert {
            after: Row(after),
            height: Px(height),
        })
        .collect();
    let folded: Vec<crate::metrics::Fold> = folded
        .iter()
        .map(|&(first, last)| crate::metrics::Fold {
            first: Row(first),
            last: Row(last),
        })
        .collect();
    RowLayout::new(LayoutParams {
        fit,
        lower: 0,
        inserts: &inserts,
        folded: &folded,
        pin,
        screen: Screen::Primary,
    })
}

#[test]
fn folded_rows_take_no_room_and_the_rows_below_them_move_up() {
    // Rows 2 and 3 are folded: the 9 rows are 5 + 20 * row. Bottom pin keeps the last row where
    // it was, so the rows above the fold move down by the two rows' height.
    let got = folded_layout(Pin::Bottom, &[(2, 3)], &[]);
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
    // Two slices (above and below the fold), none for the folded rows, no gap.
    let slices = got.slices();
    assert_eq!(slices.len(), 2, "{slices:?}");
    assert!(got.gaps().is_empty());
    let shown: u32 = slices.iter().map(|s| s.height.0).sum();
    assert_eq!(shown, 200 - 40, "the whole picture less the folded rows");
}

#[test]
fn a_top_pin_leaves_the_rows_above_a_fold_in_place() {
    let got = folded_layout(Pin::Top, &[(2, 3)], &[]);
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
    let got = folded_layout(Pin::Top, &[(2, 3)], &[]);
    // Shown y 45 is the first pixel under the fold: it is row 4's first pixel, drawn at 85.
    assert_eq!(got.to_grid(SurfacePoint { x: 7, y: 45 }), grid(85));
    assert_eq!(got.to_grid(SurfacePoint { x: 7, y: 44 }), grid(44));
}

#[test]
fn room_under_a_folded_row_is_left_out_and_a_fold_alone_does_not_make_a_gap() {
    let got = folded_layout(Pin::Top, &[(2, 3)], &[(3, 40), (6, 20)]);
    assert_eq!(got.gaps().len(), 1, "{:?}", got.gaps());
    assert_eq!(got.gaps()[0].after, Row(6));
}

#[test]
fn the_alternate_screen_never_folds() {
    let metrics = CellMetrics {
        cell: CellPx::new(10, 20),
        baseline: Px(15),
        underline: Px(16),
        strikeout: Px(10),
        thickness: Px(1),
    };
    let fit = fit_grid(PixelSize::new(100, 200), &metrics, Padding::uniform(Px(5)));
    let folded = [crate::metrics::Fold {
        first: Row(2),
        last: Row(3),
    }];
    let got = RowLayout::new(LayoutParams {
        fit,
        lower: 0,
        inserts: &[],
        folded: &folded,
        pin: Pin::Bottom,
        screen: Screen::Alternate,
    });
    assert!(!got.has_folds());
}
