use temor_vt::{CellPx, GridSize, Px, Row};

use super::*;
use crate::metrics::{CellPos, fit_grid};
use crate::renderer::SelectionShape;

fn key() -> ViewKey {
    ViewKey {
        size: PixelSize::new(80, 80),
        scale: Scale::default(),
        metrics: CellMetrics {
            cell: CellPx::new(8, 16),
            baseline: Px(12),
            underline: Px(14),
            strikeout: Px(8),
            thickness: Px(1),
        },
        padding: Padding::default(),
        grid: GridSize::new(10, 5),
        palette: Palette::default(),
        colors: TermColors::default(),
        focus: PaneFocus::Focused,
    }
}

fn select(from: (u16, u16), to: (u16, u16), shape: SelectionShape) -> Option<SelectionOverlay> {
    Some(SelectionOverlay {
        start: CellPos::new(from.0, from.1),
        end: CellPos::new(to.0, to.1),
        shape,
    })
}

/// The rows composed after `before`, then `after`, with nothing else changing.
fn composed(before: Option<SelectionOverlay>, after: Option<SelectionOverlay>) -> Compose {
    let mut tracker = ChangeTracker::default();
    tracker.plan(key(), Vec::new(), None, before, &[], &[]);
    tracker.plan(key(), Vec::new(), None, after, &[], &[])
}

fn rows(of: &[u16]) -> Compose {
    Compose::Rows(of.iter().copied().map(Row).collect())
}

#[test]
fn a_selection_change_composes_only_the_rows_it_changes() {
    use SelectionShape::{Block, Linear};
    // (name, before, after, rows composed)
    let cases = [
        ("nothing selected, still nothing", None, None, rows(&[])),
        (
            "the same selection",
            select((2, 1), (6, 3), Linear),
            select((2, 1), (6, 3), Linear),
            rows(&[]),
        ),
        (
            "a new selection on one row",
            None,
            select((2, 2), (5, 2), Linear),
            rows(&[2]),
        ),
        (
            "a selection cleared",
            select((0, 1), (4, 3), Linear),
            None,
            rows(&[1, 2, 3]),
        ),
        (
            "dragging on to the next line adds that line and the end of this one",
            select((2, 1), (5, 1), Linear),
            select((2, 1), (3, 2), Linear),
            rows(&[1, 2]),
        ),
        (
            "dragging back within a row changes only that row",
            select((2, 1), (7, 3), Linear),
            select((2, 1), (6, 3), Linear),
            rows(&[3]),
        ),
        (
            "a middle row stays when only the first row's start moves",
            select((2, 0), (5, 3), Linear),
            select((4, 0), (5, 3), Linear),
            rows(&[0]),
        ),
        (
            "a block widened changes each of its rows",
            select((2, 1), (4, 3), Block),
            select((2, 1), (6, 3), Block),
            rows(&[1, 2, 3]),
        ),
        (
            "the same cells in the other direction are the same selection",
            select((6, 3), (2, 1), Linear),
            select((2, 1), (6, 3), Linear),
            rows(&[]),
        ),
        (
            "a selection past the grid reaches only the rows it has",
            None,
            select((0, 3), (9, 40), Linear),
            rows(&[3, 4]),
        ),
    ];
    for (name, before, after, want) in cases {
        assert_eq!(composed(before, after), want, "{name}");
    }
}

#[test]
fn a_selection_change_adds_to_the_rows_that_are_dirty() {
    let mut tracker = ChangeTracker::default();
    tracker.plan(key(), Vec::new(), None, None, &[], &[]);
    let plan = tracker.plan(
        key(),
        vec![Row(0)],
        None,
        select((1, 3), (2, 3), SelectionShape::Linear),
        &[],
        &[],
    );
    assert_eq!(plan, rows(&[0, 3]));
}

#[test]
fn damage_bands_of_the_first_and_last_rows_take_in_the_outer_padding() {
    let m = key().metrics;
    // Five rows of 16 in a pane with 4 of padding on every side: grid rows span y 4..84.
    let fit = fit_grid(PixelSize::new(88, 88), &m, Padding::uniform(Px(4)));
    // (name, rows composed, (y, height) of each damage band)
    type Case<'a> = (&'a str, &'a [u16], &'a [(u32, u32)]);
    let cases: [Case; 5] = [
        ("the first row reaches the top edge", &[0], &[(0, 20)]),
        ("a middle row is its own", &[2], &[(36, 16)]),
        ("the last row reaches the bottom edge", &[4], &[(68, 20)]),
        ("neighbours merge", &[0, 1], &[(0, 36)]),
        ("all rows are one band", &[0, 1, 2, 3, 4], &[(0, 88)]),
    ];
    for (name, composed, want) in cases {
        let rows: Vec<Row> = composed.iter().copied().map(Row).collect();
        let got: Vec<(u32, u32)> = damage(Reach::Rows, &rows, &m, &fit, 5)
            .iter()
            .map(|d| (d.y.0, d.height.0))
            .collect();
        assert_eq!(got, want, "{name}");
    }
}
