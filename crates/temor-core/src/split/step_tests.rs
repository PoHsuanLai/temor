use super::tests::{l_shape, p};
use super::{Direction, Goto, TreeAfter, TreeIn, TreeOut, TreeState, tree_step};

#[test]
fn tree_step_moves_focus_with_the_commands() {
    let start = TreeState::single(p(1));
    let split = TreeIn::Split {
        towards: Direction::Right,
        new: p(2),
    };
    let (TreeAfter::Open(two), out) = tree_step(start.clone(), split) else {
        panic!()
    };
    assert_eq!((two.focus, out), (p(2), vec![TreeOut::Focused(p(2))]));

    #[rustfmt::skip]
    let cases: &[(&str, TreeIn, Option<u64>, &[TreeOut])] = &[
        ("closing the focused pane focuses the one before", TreeIn::Close(p(2)), Some(1), &[TreeOut::Focused(p(1))]),
        ("closing another keeps focus",  TreeIn::Close(p(1)), Some(2), &[]),
        ("goto moves focus",             TreeIn::Goto(Goto::Previous), Some(1), &[TreeOut::Focused(p(1))]),
        ("goto nowhere stays",           TreeIn::Goto(Goto::Next), Some(2), &[]),
        ("focus an unknown pane stays",  TreeIn::Focus(p(9)), Some(2), &[]),
        ("zoom focuses its pane",        TreeIn::Zoom(p(1)), Some(1), &[TreeOut::Focused(p(1))]),
        ("equalize keeps focus",         TreeIn::Equalize, Some(2), &[]),
    ];
    for (name, input, focus, outs) in cases {
        let (after, out) = tree_step(two.clone(), *input);
        let got = match after {
            TreeAfter::Open(state) => Some(state.focus.0),
            TreeAfter::Emptied => None,
        };
        assert_eq!(got, *focus, "{name}: focus");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
    }
    assert_eq!(
        tree_step(start, TreeIn::Close(p(1))),
        (TreeAfter::Emptied, vec![])
    );
}

#[test]
fn goto_unzooms_and_a_full_tab_refuses_a_split() {
    let zoomed = TreeState {
        tree: l_shape().zoom(p(1)).unwrap(),
        focus: p(1),
    };
    let (TreeAfter::Open(state), _) = tree_step(zoomed, TreeIn::Goto(Goto::Next)) else {
        panic!()
    };
    assert_eq!((state.focus, state.tree.zoomed()), (p(2), None));

    let full =
        (2..super::MOST_PANES as u64 + 1).fold(
            TreeState::single(p(1)),
            |state, n| match tree_step(
                state,
                TreeIn::Split {
                    towards: Direction::Down,
                    new: p(n),
                },
            ) {
                (TreeAfter::Open(state), _) => state,
                (TreeAfter::Emptied, _) => panic!(),
            },
        );
    assert_eq!(full.tree.panes().len(), super::MOST_PANES);
    let more = TreeIn::Split {
        towards: Direction::Down,
        new: p(1000),
    };
    let (after, out) = tree_step(full.clone(), more);
    assert_eq!(
        (after, out),
        (TreeAfter::Open(full), vec![TreeOut::SplitRefused])
    );
}

#[test]
fn focusing_another_pane_while_zoomed_unzooms() {
    let zoomed = TreeState {
        tree: l_shape().zoom(p(1)).unwrap(),
        focus: p(1),
    };
    let (TreeAfter::Open(state), out) = tree_step(zoomed.clone(), TreeIn::Focus(p(3))) else {
        panic!()
    };
    assert_eq!(
        (state.focus, state.tree.zoomed(), out),
        (p(3), None, vec![TreeOut::Focused(p(3))])
    );
    let (TreeAfter::Open(same), _) = tree_step(zoomed, TreeIn::Focus(p(1))) else {
        panic!()
    };
    assert_eq!(
        same.tree.zoomed(),
        Some(p(1)),
        "focusing the zoomed pane keeps the zoom"
    );
}

/// Closing a pane gives focus to the pane it was split from or beside, not to whichever comes
/// next in reading order: with 1 above 2 and 3 split off 1's right, closing 3 focuses 1.
#[test]
fn closing_a_pane_focuses_the_one_it_was_split_beside() {
    let tree = super::SplitTree::single(p(1))
        .split(p(1), p(2), Direction::Down)
        .and_then(|t| t.split(p(1), p(3), Direction::Right))
        .unwrap();
    let state = TreeState { tree, focus: p(3) };
    let (TreeAfter::Open(after), out) = tree_step(state, TreeIn::Close(p(3))) else {
        panic!("one pane closed of three")
    };
    assert_eq!((after.focus, out), (p(1), vec![TreeOut::Focused(p(1))]));
}
