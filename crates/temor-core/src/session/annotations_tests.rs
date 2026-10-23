//! The annotations' step: the picks, bookmarks and folds a person's keys and clicks make.

use std::collections::BTreeSet;

use super::{AnnotateIn, AnnotateOut, Annotations, annotate_step};
use crate::block::{PickClick, PickStep};
use crate::test_support::run_steps;
use crate::vocab::ids::BlockId;

fn order() -> Vec<BlockId> {
    (1..=5).map(BlockId).collect()
}

fn ids(state: &Annotations) -> Vec<u64> {
    state.picked_in(&order()).iter().map(|b| b.0).collect()
}

fn click(id: u64, how: PickClick) -> AnnotateIn {
    AnnotateIn::Click {
        id: BlockId(id),
        how,
        order: order(),
    }
}

fn foldable(of: &[u64]) -> BTreeSet<BlockId> {
    of.iter().copied().map(BlockId).collect()
}

#[test]
fn clicks_pick_and_arrows_move_the_pick_and_ask_to_reveal_it() {
    let (state, outs) = run_steps(
        Annotations::default(),
        [
            click(2, PickClick::Only),
            click(4, PickClick::Range),
            AnnotateIn::Step {
                step: PickStep::Older,
                order: order(),
            },
        ],
        annotate_step,
    );
    assert_eq!(
        ids(&state),
        [3],
        "the arrow leaves one block, above the active one"
    );
    assert_eq!(outs, [AnnotateOut::Reveal], "only the arrow asks to reveal");
}

#[test]
fn clearing_and_selecting_all() {
    let (state, _) = run_steps(
        Annotations::default(),
        [AnnotateIn::SelectAll { order: order() }],
        annotate_step,
    );
    assert_eq!(ids(&state), [1, 2, 3, 4, 5]);
    let (state, _) = annotate_step(state, AnnotateIn::ClearPicks);
    assert!(state.nothing_picked());
}

#[test]
fn bookmarks_toggle_for_the_picked_blocks_and_the_pick_steps_between_them() {
    let (state, _) = run_steps(
        Annotations::default(),
        [
            click(2, PickClick::Only),
            AnnotateIn::ToggleBookmarks { order: order() },
            click(4, PickClick::Only),
            AnnotateIn::ToggleBookmarks { order: order() },
        ],
        annotate_step,
    );
    assert!(state.is_bookmarked(BlockId(2)) && state.is_bookmarked(BlockId(4)));
    assert!(state.picks_bookmarked(&order()));
    let (state, outs) = annotate_step(
        state,
        AnnotateIn::StepBookmark {
            step: PickStep::Older,
            order: order(),
        },
    );
    assert_eq!(ids(&state), [2], "the nearest bookmark above the pick");
    assert_eq!(outs, [AnnotateOut::Reveal]);
    let (state, outs) = annotate_step(
        state,
        AnnotateIn::StepBookmark {
            step: PickStep::Older,
            order: order(),
        },
    );
    assert_eq!(
        (ids(&state), outs),
        (vec![2], vec![]),
        "none that way: the pick stays"
    );
    let (state, _) = annotate_step(state, AnnotateIn::ToggleBookmarks { order: order() });
    assert!(
        !state.is_bookmarked(BlockId(2)),
        "every picked one had one: they go"
    );
    assert!(state.is_bookmarked(BlockId(4)));
}

#[test]
fn toggling_bookmarks_with_nothing_picked_changes_nothing() {
    let (state, _) = annotate_step(
        Annotations::default(),
        AnnotateIn::ToggleBookmarks { order: order() },
    );
    assert_eq!(state, Annotations::default());
}

#[test]
fn folding_the_picked_blocks_that_have_output_and_unfolding_them() {
    let fold = AnnotateIn::ToggleFolds {
        order: order(),
        foldable: foldable(&[2, 3]),
    };
    let (state, outs) = run_steps(
        Annotations::default(),
        [
            click(1, PickClick::Only),
            click(3, PickClick::Range),
            fold.clone(),
        ],
        annotate_step,
    );
    assert!(
        !state.is_folded(BlockId(1)),
        "block 1 has no output to fold"
    );
    assert!(state.is_folded(BlockId(2)) && state.is_folded(BlockId(3)));
    assert_eq!(outs, [AnnotateOut::FoldsChanged]);
    assert!(state.picks_folded(&order(), &foldable(&[2, 3])));
    let (state, _) = annotate_step(state, fold);
    assert_eq!(
        state.folded().count(),
        0,
        "every foldable pick was folded: they open"
    );
}

#[test]
fn one_block_folds_only_when_it_has_output_and_always_opens() {
    let toggle = |id, foldable| AnnotateIn::ToggleFold {
        id: BlockId(id),
        foldable,
    };
    let (state, outs) = annotate_step(Annotations::default(), toggle(2, false));
    assert_eq!(
        (state.folded().count(), outs),
        (0, vec![]),
        "nothing to fold"
    );
    let (state, outs) = annotate_step(state, toggle(2, true));
    assert_eq!(
        (state.is_folded(BlockId(2)), outs),
        (true, vec![AnnotateOut::FoldsChanged])
    );
    let (state, outs) = annotate_step(state, toggle(2, false));
    assert_eq!(
        (state.is_folded(BlockId(2)), outs),
        (false, vec![AnnotateOut::FoldsChanged]),
        "a folded block opens even when it no longer has output"
    );
}
