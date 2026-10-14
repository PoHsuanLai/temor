use temor_vt::{CellPx, GridSize, Px, Row};

use super::*;
use crate::metrics::CellPos;
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
