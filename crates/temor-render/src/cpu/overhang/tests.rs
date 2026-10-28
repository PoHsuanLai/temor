use std::convert::Infallible;

/// Whether a row's glyphs reach past it.
const BEYOND: bool = true;
const WITHIN: bool = false;
use super::*;
use crate::font::Slot;
use crate::scene::{Premul, Quad};

fn rows(of: &[u16]) -> Vec<Row> {
    of.iter().copied().map(Row).collect()
}

/// Composes `dirty` of 8 rows where `now` says which rows overhang and `before` which did.
fn composed(before: &[u16], now: &[u16], dirty: &[u16]) -> Vec<Row> {
    let mut state = Overhangs::default();
    state.fit(8);
    for &r in before {
        state.replace(Row(r), BEYOND);
    }
    let mut asked = Vec::new();
    let got = compose_with_neighbours::<Infallible>(&rows(dirty), 8, &mut state, |row, _| {
        asked.push(row);
        Ok(if now.contains(&row.0) { BEYOND } else { WITHIN })
    })
    .expect("composes");
    assert_eq!(
        got,
        {
            let mut sorted = asked.clone();
            sorted.sort();
            sorted
        },
        "the rows returned are the rows composed"
    );
    got
}

#[test]
fn a_changed_row_takes_its_neighbours_only_when_its_glyphs_overhang() {
    // (name, rows that overhung, rows that overhang now, dirty rows) -> rows composed
    let cases = [
        ("no overhang anywhere", vec![], vec![], vec![3], vec![3]),
        ("overhang now", vec![], vec![3], vec![3], vec![2, 3, 4]),
        (
            "overhang before, none now",
            vec![3],
            vec![],
            vec![3],
            vec![2, 3, 4],
        ),
        (
            "the top row has no row above",
            vec![],
            vec![0],
            vec![0],
            vec![0, 1],
        ),
        (
            "the bottom row has no row below",
            vec![],
            vec![7],
            vec![7],
            vec![6, 7],
        ),
        (
            "a neighbour that overhangs does not pull in its own",
            vec![],
            vec![2],
            vec![3],
            vec![3],
        ),
        (
            "two dirty rows share their neighbour",
            vec![],
            vec![3, 4],
            vec![3, 4],
            vec![2, 3, 4, 5],
        ),
    ];
    for (name, before, now, dirty, want) in cases {
        assert_eq!(composed(&before, &now, &dirty), rows(&want), "{name}");
    }
}

#[test]
fn the_overhang_of_a_row_is_remembered_for_the_next_change() {
    let mut state = Overhangs::default();
    state.fit(4);
    let mut run = |now: bool, dirty: u16| {
        compose_with_neighbours::<Infallible>(&rows(&[dirty]), 4, &mut state, |_, _| Ok(now))
            .expect("composes")
            .len()
    };
    assert_eq!(run(BEYOND, 1), 3, "it overhangs now");
    assert_eq!(
        run(WITHIN, 1),
        3,
        "it overhung last time, so its neighbours are cleaned"
    );
    assert_eq!(run(WITHIN, 1), 1, "and now it is within the row for good");
}

fn quad(y: i32, h: u32, paint: QuadPaint) -> Quad {
    Quad {
        x: 0,
        y,
        w: 4,
        h,
        paint,
    }
}

#[test]
fn only_glyphs_that_leave_the_row_overhang() {
    let slot = Slot::default();
    let mask = QuadPaint::Mask {
        slot,
        color: Premul([255; 4]),
    };
    let solid = QuadPaint::Solid(Premul([255; 4]));
    // The row spans y 100..120. (name, glyph quad y, h, paint) -> overhang
    let cases = [
        ("inside", quad(102, 10, mask), WITHIN),
        ("filling the row", quad(100, 20, mask), WITHIN),
        ("above", quad(98, 10, mask), BEYOND),
        ("below", quad(112, 10, mask), BEYOND),
        (
            "a colour glyph below",
            quad(112, 10, QuadPaint::Color { slot }),
            BEYOND,
        ),
        ("a rule is not a glyph", quad(90, 40, solid), WITHIN),
    ];
    for (name, q, want) in cases {
        let scene = RowScene {
            bg: Vec::new(),
            fg: vec![q],
        };
        assert_eq!(overhang_of(&scene, 100, 20), want, "{name}");
    }
}

/// The `Sides` each composed row is told, for 6 rows where `kept` overhung when last composed,
/// `now` overhang now, and `dirty` are asked.
fn told(kept: &[u16], now: &[u16], dirty: &[u16]) -> Vec<(u16, bool, bool)> {
    let mut state = Overhangs::default();
    state.fit(6);
    for &r in kept {
        state.replace(Row(r), BEYOND);
    }
    let mut seen = Vec::new();
    compose_with_neighbours::<Infallible>(&rows(dirty), 6, &mut state, |row, sides| {
        seen.push((row.0, sides.above, sides.below));
        Ok(if now.contains(&row.0) { BEYOND } else { WITHIN })
    })
    .expect("composes");
    seen
}

#[test]
fn a_row_is_told_which_neighbours_may_reach_into_it() {
    // (name, overhung before, overhang now, dirty) -> (row, above, below) for each row composed
    let cases = [
        (
            "nothing overhangs: no neighbour is built",
            vec![],
            vec![],
            vec![2],
            vec![(2, WITHIN, WITHIN)],
        ),
        (
            "a clean neighbour that overhung is built",
            vec![1],
            vec![1],
            vec![2],
            vec![(2, BEYOND, WITHIN)],
        ),
        (
            "the grid's edges have no neighbour to build",
            vec![],
            vec![],
            vec![0, 5],
            vec![(0, WITHIN, WITHIN), (5, WITHIN, WITHIN)],
        ),
        (
            "a neighbour still to be composed is built, whatever it was",
            vec![],
            vec![],
            vec![2, 3],
            vec![(2, WITHIN, BEYOND), (3, WITHIN, WITHIN)],
        ),
        (
            "a neighbour composed already is told by what it is now",
            vec![],
            vec![2],
            vec![2, 3],
            vec![
                (2, WITHIN, BEYOND),
                (3, BEYOND, WITHIN),
                (1, WITHIN, BEYOND),
            ],
        ),
    ];
    for (name, kept, now, dirty, want) in cases {
        let mut got = told(&kept, &now, &dirty);
        let mut want = want;
        got.sort_by_key(|t| t.0);
        want.sort_by_key(|t| t.0);
        assert_eq!(got, want, "{name}");
    }
}
