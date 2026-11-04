use super::tests::{l_shape, p};
use super::{Axis, Direction, MOST_PANES, Node, SplitTree};
use temor_core::error::CoreError;

fn single(n: u64) -> SplitTree {
    SplitTree::single(p(n))
}

#[test]
fn graft_puts_the_other_tree_on_the_side_asked() {
    #[rustfmt::skip]
    type Case = (&'static str, SplitTree, u64, Direction, &'static [u64], Axis);
    let cases: &[Case] = &[
        (
            "right of 1",
            single(1),
            1,
            Direction::Right,
            &[1, 2],
            Axis::Row,
        ),
        (
            "left of 1",
            single(1),
            1,
            Direction::Left,
            &[2, 1],
            Axis::Row,
        ),
        (
            "down of 1",
            single(1),
            1,
            Direction::Down,
            &[1, 2],
            Axis::Column,
        ),
        (
            "up of 1",
            single(1),
            1,
            Direction::Up,
            &[2, 1],
            Axis::Column,
        ),
        (
            "right of 2 in an L",
            l_shape(),
            2,
            Direction::Right,
            &[1, 2, 4, 3],
            Axis::Column,
        ),
    ];
    for (name, host, at, towards, order, axis) in cases {
        let other = if *name == "right of 2 in an L" {
            single(4)
        } else {
            single(2)
        };
        let got = host.graft(p(*at), &other, *towards).unwrap();
        assert_eq!(
            got.panes(),
            order.iter().map(|n| p(*n)).collect::<Vec<_>>(),
            "{name}"
        );
        assert!(got.is_split(), "{name}");
        assert!(
            got.nodes()
                .iter()
                .any(|n| matches!(n, Node::Split { axis: a, .. } if a == axis)),
            "{name}: axis"
        );
    }
}

#[test]
fn graft_of_a_split_tree_keeps_its_shape() {
    let other = l_shape().split(p(2), p(9), Direction::Right).unwrap();
    let got = single(8).graft(p(8), &other, Direction::Right).unwrap();
    let back = got.tear(p(8)).unwrap().0;
    assert_eq!(back.panes().len(), other.panes().len());
    assert_eq!(got.panes().len(), other.panes().len() + 1);
}

#[test]
fn graft_refusals_name_the_cause() {
    let full = (2..=MOST_PANES as u64).fold(single(1), |tree, n| {
        tree.split(p(n - 1), p(n), Direction::Right).unwrap()
    });
    #[rustfmt::skip]
    let cases: Vec<(&str, Result<SplitTree, CoreError>)> = vec![
        ("unknown host pane", l_shape().graft(p(99), &single(7), Direction::Right)),
        ("onto itself", l_shape().graft(p(1), &l_shape(), Direction::Right)),
        ("shares one pane", l_shape().graft(p(1), &single(3), Direction::Right)),
        ("past the pane limit", full.graft(p(1), &single(200), Direction::Right)),
    ];
    let want: [CoreError; 4] = [
        CoreError::UnknownPane(99),
        CoreError::DuplicatePane(1),
        CoreError::DuplicatePane(3),
        CoreError::TreeFull,
    ];
    for ((name, got), want) in cases.into_iter().zip(want) {
        assert_eq!(got, Err(want), "{name}");
    }
}

#[test]
fn tear_splits_a_pane_out_and_refuses_the_last_pane() {
    let (rest, alone) = l_shape().tear(p(2)).unwrap();
    assert_eq!(rest.panes(), vec![p(1), p(3)]);
    assert_eq!(alone.panes(), vec![p(2)]);
    assert_eq!(single(5).tear(p(5)), Err(CoreError::LastPane));
    assert_eq!(l_shape().tear(p(42)), Err(CoreError::UnknownPane(42)));
}

#[test]
fn tear_then_graft_back_restores_the_panes() {
    let tree = l_shape();
    let (rest, alone) = tree.tear(p(2)).unwrap();
    let back = rest.graft(p(1), &alone, Direction::Right).unwrap();
    assert_eq!(back.panes().len(), tree.panes().len());
    assert!(back.contains(p(2)));
}
