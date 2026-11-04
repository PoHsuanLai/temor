//! Property tests for the split tree: reachability, split/remove inverse, equalize idempotent,
//! resize clamped, spatial goto symmetric where the geometry allows, and exact tiling.

use ds_core::word::Word;
use proptest::prelude::*;
use temor_core::vocab::ids::PaneId;
use temor_layout::split::{
    Axis, Closed, Direction, Goto, Node, Share, SplitTree, WholePx, WindowDelta,
};

#[derive(Debug, Clone)]
enum Op {
    Split(usize, Direction),
    Remove(usize),
    Resize(usize, Axis, i32),
}

fn op() -> impl Strategy<Value = Op> {
    let direction = prop::sample::select(Direction::ALL.to_vec());
    let axis = prop::sample::select(Axis::ALL.to_vec());
    prop_oneof![
        3 => (any::<usize>(), direction).prop_map(|(at, d)| Op::Split(at, d)),
        1 => any::<usize>().prop_map(Op::Remove),
        2 => (any::<usize>(), axis, -2_000_000i32..2_000_000).prop_map(|(at, a, d)| Op::Resize(at, a, d)),
    ]
}

fn build(ops: &[Op]) -> SplitTree {
    let mut tree = SplitTree::single(PaneId(0));
    let mut next = 1;
    for op in ops {
        let panes = tree.panes();
        let pick = |at: usize| panes[at % panes.len()];
        tree = match *op {
            Op::Split(at, towards) => {
                next += 1;
                tree.split(pick(at), PaneId(next), towards).unwrap_or(tree)
            }
            Op::Remove(at) => match tree.remove(pick(at)) {
                Ok(Closed::Remaining(rest)) => rest,
                _ => tree,
            },
            Op::Resize(at, axis, delta) => tree.resize(pick(at), axis, WindowDelta(delta)).unwrap(),
        };
    }
    tree
}

fn tree() -> impl Strategy<Value = SplitTree> {
    prop::collection::vec(op(), 0..40).prop_map(|ops| build(&ops))
}

fn ratios(tree: &SplitTree) -> Vec<u32> {
    tree.nodes()
        .iter()
        .filter_map(|node| match node {
            Node::Split { ratio, .. } => Some(ratio.millionths()),
            Node::Leaf(_) => None,
        })
        .collect()
}

/// The panes reached by walking from the root, and how many nodes the walk visited.
fn walk(tree: &SplitTree) -> (Vec<PaneId>, usize) {
    let (mut panes, mut visited, mut stack) = (Vec::new(), 0, vec![0usize]);
    while let Some(at) = stack.pop() {
        visited += 1;
        match tree.nodes()[at] {
            Node::Leaf(pane) => panes.push(pane),
            Node::Split { first, second, .. } => {
                stack.push(second.index());
                stack.push(first.index());
            }
        }
    }
    (panes, visited)
}

/// `columns` side by side, each split into `rows` stacked, all shares even.
fn grid(columns: u64, rows: u64) -> SplitTree {
    let mut tree = SplitTree::single(PaneId(0));
    for c in 1..columns {
        tree = tree
            .split(PaneId((c - 1) * rows), PaneId(c * rows), Direction::Right)
            .unwrap();
    }
    for c in 0..columns {
        for r in 1..rows {
            let above = PaneId(c * rows + r - 1);
            tree = tree
                .split(above, PaneId(c * rows + r), Direction::Down)
                .unwrap();
        }
    }
    tree.equalize()
}

/// Whether `there` lies wholly on `d`'s side of `from` on screen.
fn lies_beyond(tree: &SplitTree, from: PaneId, there: PaneId, d: Direction) -> bool {
    let rects = tree.layout(WholePx(1 << 20), WholePx(1 << 20));
    let rect = |pane| {
        rects
            .iter()
            .find(|(p, _)| *p == pane)
            .map(|(_, r)| *r)
            .unwrap()
    };
    let (a, b) = (rect(from), rect(there));
    match d {
        Direction::Left => b.x.0 + b.width.0 <= a.x.0,
        Direction::Right => b.x.0 >= a.x.0 + a.width.0,
        Direction::Up => b.y.0 + b.height.0 <= a.y.0,
        Direction::Down => b.y.0 >= a.y.0 + a.height.0,
    }
}

proptest! {
    #[test]
    fn every_leaf_is_reachable_from_the_root(tree in tree()) {
        let (reached, visited) = walk(&tree);
        prop_assert_eq!(&reached, &tree.panes());
        prop_assert_eq!(visited, tree.nodes().len());
        prop_assert_eq!(tree.nodes().len(), 2 * reached.len() - 1);
    }

    #[test]
    fn split_then_remove_gives_back_the_original(tree in tree(), at in any::<usize>(), d in 0..Direction::ALL.len()) {
        let panes = tree.panes();
        let pane = panes[at % panes.len()];
        let new = PaneId(u64::MAX);
        let split = tree.split(pane, new, Direction::ALL[d]).unwrap();
        prop_assert_eq!(split.panes().len(), panes.len() + 1);
        prop_assert_eq!(split.remove(new), Ok(Closed::Remaining(tree)));
    }

    #[test]
    fn equalize_is_idempotent(tree in tree()) {
        let once = tree.equalize();
        prop_assert_eq!(once.equalize(), once);
    }

    #[test]
    fn resize_stays_clamped(tree in tree(), at in any::<usize>(), delta in any::<i32>(), row in any::<bool>()) {
        let panes = tree.panes();
        let axis = if row { Axis::Row } else { Axis::Column };
        let resized = tree.resize(panes[at % panes.len()], axis, WindowDelta(delta)).unwrap();
        for ratio in ratios(&resized) {
            prop_assert!((Share::LEAST..=Share::WHOLE - Share::LEAST).contains(&ratio), "{ratio}");
        }
    }

    #[test]
    fn spatial_goto_is_symmetric_in_a_strip(ops in prop::collection::vec((any::<usize>(), -400_000i32..400_000), 1..12), row in any::<bool>()) {
        let (forward, axis) = if row { (Direction::Right, Axis::Row) } else { (Direction::Down, Axis::Column) };
        let mut strip = SplitTree::single(PaneId(0));
        for (n, (at, delta)) in ops.iter().enumerate() {
            let panes = strip.panes();
            let pane = panes[at % panes.len()];
            strip = strip.split(pane, PaneId(n as u64 + 1), forward).unwrap();
            strip = strip.resize(pane, axis, WindowDelta(*delta)).unwrap();
        }
        let panes = strip.panes();
        for (i, pane) in panes.iter().enumerate() {
            let next = panes[(i + 1) % panes.len()];
            prop_assert_eq!(strip.goto(*pane, Goto::Spatial(forward)), Some(next));
            prop_assert_eq!(strip.goto(next, Goto::Spatial(forward.opposite())), Some(*pane));
        }
    }

    #[test]
    fn spatial_goto_is_symmetric_in_an_even_grid(columns in 1u64..6, rows in 1u64..6) {
        let grid = grid(columns, rows);
        for pane in grid.panes() {
            for &d in Direction::ALL {
                // A move round the edge lands by Ghostty's wrapped-corner rule, which has no
                // inverse; a move to a neighbour does.
                let there = grid.goto(pane, Goto::Spatial(d)).filter(|there| lies_beyond(&grid, pane, *there, d));
                if let Some(there) = there {
                    prop_assert_eq!(grid.goto(there, Goto::Spatial(d.opposite())), Some(pane), "{:?} {:?}", pane, d);
                }
            }
        }
    }

    #[test]
    fn the_rectangles_tile_the_area_exactly(tree in tree(), width in 1u32..5000, height in 1u32..5000) {
        let rects = tree.layout(WholePx(width), WholePx(height));
        prop_assert_eq!(rects.len(), tree.panes().len());
        let area: u64 = rects.iter().map(|(_, r)| u64::from(r.width.0) * u64::from(r.height.0)).sum();
        prop_assert_eq!(area, u64::from(width) * u64::from(height));
        for (i, (_, a)) in rects.iter().enumerate() {
            prop_assert!(a.x.0 + a.width.0 <= width && a.y.0 + a.height.0 <= height);
            for (_, b) in &rects[i + 1..] {
                let apart = a.x.0 + a.width.0 <= b.x.0 || b.x.0 + b.width.0 <= a.x.0
                    || a.y.0 + a.height.0 <= b.y.0 || b.y.0 + b.height.0 <= a.y.0;
                prop_assert!(apart, "{:?} overlaps {:?}", a, b);
            }
        }
    }

    #[test]
    fn every_rectangle_is_non_empty_when_the_tab_has_room(tree in tree(), width in 128u32..5000, height in 128u32..5000) {
        for (pane, rect) in tree.layout(WholePx(width), WholePx(height)) {
            prop_assert!(rect.width.0 > 0 && rect.height.0 > 0, "{:?} {:?}", pane, rect);
        }
    }

    #[test]
    fn a_tree_round_trips_through_its_stored_form(tree in tree()) {
        let text = serde_json::to_string(&tree).unwrap();
        prop_assert_eq!(serde_json::from_str::<SplitTree>(&text).unwrap(), tree);
    }
}
