use super::{Axis, Closed, Direction, Goto, Node, Share, SplitTree, WholePx, WindowDelta};
use crate::error::CoreError;
use crate::vocab::ids::PaneId;

pub(super) fn p(n: u64) -> PaneId {
    PaneId(n)
}

/// Pane 1 split right into 2, then 2 split down into 3:
/// ```text
/// +---+---+
/// |   | 2 |
/// | 1 +---+
/// |   | 3 |
/// +---+---+
/// ```
pub(super) fn l_shape() -> SplitTree {
    SplitTree::single(p(1))
        .split(p(1), p(2), Direction::Right)
        .and_then(|t| t.split(p(2), p(3), Direction::Down))
        .unwrap()
}

fn ratio_at(tree: &SplitTree, index: usize) -> u32 {
    match tree.nodes()[index] {
        Node::Split { ratio, .. } => ratio.millionths(),
        Node::Leaf(_) => panic!("node {index} is a leaf"),
    }
}

#[test]
fn split_places_the_new_pane_on_the_side_asked() {
    #[rustfmt::skip]
    let cases: &[(&str, Direction, &[u64], Axis)] = &[
        ("right puts it second", Direction::Right, &[1, 2], Axis::Row),
        ("left puts it first",   Direction::Left,  &[2, 1], Axis::Row),
        ("down puts it second",  Direction::Down,  &[1, 2], Axis::Column),
        ("up puts it first",     Direction::Up,    &[2, 1], Axis::Column),
    ];
    for (name, towards, order, axis) in cases {
        let tree = SplitTree::single(p(1)).split(p(1), p(2), *towards).unwrap();
        let panes: Vec<u64> = tree.panes().iter().map(|pane| pane.0).collect();
        assert_eq!(panes, *order, "{name}");
        assert!(
            matches!(tree.nodes()[0], Node::Split { axis: a, ratio, .. } if a == *axis && ratio == Share::HALF),
            "{name}: {:?}",
            tree.nodes()[0]
        );
    }
}

#[test]
fn split_refuses_unknown_and_duplicate_panes() {
    let tree = l_shape();
    assert_eq!(
        tree.split(p(9), p(10), Direction::Right),
        Err(CoreError::UnknownPane(9))
    );
    assert_eq!(
        tree.split(p(1), p(3), Direction::Right),
        Err(CoreError::DuplicatePane(3))
    );
}

#[test]
fn removing_a_pane_promotes_its_sibling() {
    let tree = l_shape();
    let Ok(Closed::Remaining(without_two)) = tree.remove(p(2)) else {
        panic!()
    };
    assert_eq!(
        without_two,
        SplitTree::single(p(1))
            .split(p(1), p(3), Direction::Right)
            .unwrap()
    );
    let Ok(Closed::Remaining(without_one)) = tree.remove(p(1)) else {
        panic!()
    };
    assert_eq!(without_one.panes(), vec![p(2), p(3)]);
    assert!(matches!(
        without_one.nodes()[0],
        Node::Split {
            axis: Axis::Column,
            ..
        }
    ));
    assert_eq!(SplitTree::single(p(1)).remove(p(1)), Ok(Closed::Last));
    assert_eq!(tree.remove(p(7)), Err(CoreError::UnknownPane(7)));
}

#[test]
fn equalize_weighs_children_along_the_split_axis() {
    // Three in a row: the root split's first child is one pane, its second is two in a row.
    let row = SplitTree::single(p(1))
        .split(p(1), p(2), Direction::Right)
        .and_then(|t| t.split(p(2), p(3), Direction::Right))
        .unwrap()
        .equalize();
    assert_eq!(ratio_at(&row, 0), 333_333, "a third for the lone pane");
    assert_eq!(ratio_at(&row, 2), 500_000, "the pair splits evenly");
    // The L: the column on the right counts as one along the row.
    assert_eq!(ratio_at(&l_shape().equalize(), 0), 500_000);
}

#[test]
fn resize_scales_a_window_delta_to_the_split_and_clamps() {
    let tree = l_shape();
    // Pane 3's column split spans the window's full height: a tenth of the window is a tenth.
    let moved = tree
        .resize(p(3), Axis::Column, WindowDelta(100_000))
        .unwrap();
    assert_eq!(ratio_at(&moved, 2), 600_000);
    // Its nearest row split is the root, the whole window.
    let moved = tree.resize(p(3), Axis::Row, WindowDelta(-100_000)).unwrap();
    assert_eq!(ratio_at(&moved, 0), 400_000);
    // In 1 | (2 | 3) the inner split is half the width: a tenth of the window is a fifth of it.
    let row = SplitTree::single(p(1))
        .split(p(1), p(2), Direction::Right)
        .and_then(|t| t.split(p(2), p(3), Direction::Right))
        .unwrap();
    let moved = row.resize(p(3), Axis::Row, WindowDelta(100_000)).unwrap();
    assert_eq!(ratio_at(&moved, 2), 700_000);
    let clamped = tree
        .resize(p(1), Axis::Row, WindowDelta(5_000_000))
        .unwrap();
    assert_eq!(ratio_at(&clamped, 0), Share::WHOLE - Share::LEAST);
    // Pane 1 has no column split above it.
    assert_eq!(
        tree.resize(p(1), Axis::Column, WindowDelta(100_000)),
        Ok(tree.clone())
    );
}

#[test]
fn zoom_needs_a_split_and_goes_with_its_pane() {
    let tree = l_shape().zoom(p(2)).unwrap();
    assert_eq!(tree.zoomed(), Some(p(2)));
    assert_eq!(tree.layout(WholePx(100), WholePx(50)).len(), 1);
    let Ok(Closed::Remaining(after)) = tree.remove(p(2)) else {
        panic!()
    };
    assert_eq!(after.zoomed(), None);
    let Ok(Closed::Remaining(after)) = tree.remove(p(3)) else {
        panic!()
    };
    assert_eq!(
        after.zoomed(),
        Some(p(2)),
        "a zoom on another pane survives"
    );
    assert_eq!(SplitTree::single(p(1)).zoom(p(1)).unwrap().zoomed(), None);
    assert_eq!(tree.unzoom().zoomed(), None);
}

#[test]
fn goto_moves_in_reading_order_and_on_screen() {
    let tree = l_shape();
    #[rustfmt::skip]
    let cases: &[(&str, u64, Goto, Option<u64>)] = &[
        ("previous from the first is none", 1, Goto::Previous, None),
        ("next from the first",             1, Goto::Next, Some(2)),
        ("next from the last is none",      3, Goto::Next, None),
        ("next wraps to the first",         3, Goto::NextWrapped, Some(1)),
        ("previous wraps to the last",      1, Goto::PreviousWrapped, Some(3)),
        ("right of 1 is the nearest corner", 1, Goto::Spatial(Direction::Right), Some(2)),
        ("left of 3 is 1",                  3, Goto::Spatial(Direction::Left), Some(1)),
        ("down from 2 is 3",                2, Goto::Spatial(Direction::Down), Some(3)),
        ("up from 2 wraps to 3",            2, Goto::Spatial(Direction::Up), Some(3)),
        ("right of 2 wraps to 1",           2, Goto::Spatial(Direction::Right), Some(1)),
        ("up from 1 wraps to the nearest corner below", 1, Goto::Spatial(Direction::Up), Some(3)),
        ("an unknown pane goes nowhere",    9, Goto::Next, None),
    ];
    for (name, from, to, want) in cases {
        assert_eq!(tree.goto(p(*from), *to), want.map(p), "{name}");
    }
}

#[test]
fn layout_gives_whole_pixel_rects_that_tile() {
    let rects = l_shape().layout(WholePx(101), WholePx(51));
    let as_tuples: Vec<(u64, [u32; 4])> = rects
        .iter()
        .map(|(pane, r)| (pane.0, [r.x.0, r.y.0, r.width.0, r.height.0]))
        .collect();
    assert_eq!(
        as_tuples,
        vec![
            (1, [0, 0, 50, 51]),
            (2, [50, 0, 51, 25]),
            (3, [50, 25, 51, 26])
        ]
    );
}

#[test]
fn a_stored_tree_must_be_one_tree_over_distinct_panes() {
    let leaf = |n| Node::Leaf(p(n));
    let split = |first, second| Node::Split {
        axis: Axis::Row,
        ratio: Share::HALF,
        first: super::Handle(first),
        second: super::Handle(second),
    };
    #[rustfmt::skip]
    let cases: &[(&str, Vec<Node>, Result<(), &str>)] = &[
        ("one leaf",            vec![leaf(1)], Ok(())),
        ("children out of order are re-laid", vec![split(2, 1), leaf(1), leaf(2)], Ok(())),
        ("empty",               vec![], Err("no panes")),
        ("a handle past the end", vec![split(1, 5), leaf(1)], Err("a handle points past the end")),
        ("a cycle",             vec![split(0, 1), leaf(1)], Err("a node is reached twice")),
        ("a pane twice",        vec![split(1, 2), leaf(1), leaf(1)], Err("a pane appears twice")),
        ("an orphan",           vec![leaf(1), leaf(2)], Err("a node is not reachable")),
    ];
    for (name, nodes, want) in cases {
        let got = SplitTree::from_nodes(nodes.clone(), None).map(|_| ());
        assert_eq!(got, want.map_err(CoreError::BadTree), "{name}");
    }
    let relaid = SplitTree::from_nodes(vec![split(2, 1), leaf(1), leaf(2)], Some(p(9))).unwrap();
    assert_eq!(relaid.panes(), vec![p(2), p(1)]);
    assert_eq!(relaid.nodes()[0], split(1, 2));
    assert_eq!(relaid.zoomed(), None, "a zoom on a missing pane is dropped");
}

#[test]
fn equalize_stays_exact_at_the_most_panes() {
    let strip = (1..super::MOST_PANES as u64).fold(SplitTree::single(p(0)), |tree, n| {
        tree.split(p(n - 1), p(n), Direction::Right).unwrap()
    });
    let even = strip.equalize();
    let want = Share::WHOLE / super::MOST_PANES as u32;
    assert_eq!(ratio_at(&even, 0), want, "one pane against the other 127");
}
