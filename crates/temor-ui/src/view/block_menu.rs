//! The menu on the picked blocks, the one menu for a block (`view::menu`): a right-click on a
//! block, or its card's "…", picks it first (keeping the pick when the block was in it), so each
//! item acts on what is tinted. To the one menu's items it adds the lenses that can draw the
//! block, and Hide under a picture a lens drew.

use dioxus::prelude::*;
use ds::host::measure::Anchor;
use ds::prelude::*;
use temor_client::model::menu::{BlockFacts, Target};

use super::menu::{Pick, menu_items};
use crate::plugins::lens::LensOffer;

/// Where the menu is up: the right-click's point in the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockMenuAt {
    pub point: Point,
}

/// The block menu's own items: a lens to draw the block with (its id), or hiding its picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockOwn {
    Lens(String),
    HidePicture,
}

/// What the menu says, read from the session when it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMenuState {
    pub facts: BlockFacts,
    /// The lenses that can draw the selected block; offered when one block is picked.
    pub lenses: Vec<LensOffer>,
    /// A lens has drawn a picture under the block: the menu offers to hide it.
    pub picture: bool,
}

/// The menu's items: the one menu's for the picked blocks, then the lenses and Hide.
pub fn block_menu_items(state: BlockMenuState) -> Vec<MenuItem<Pick<BlockOwn>>> {
    let many = state.facts.picked > 1;
    let mut own: Vec<MenuItem<BlockOwn>> = Vec::new();
    if !many {
        own.extend(state.lenses.iter().map(|lens| {
            MenuItem::new(BlockOwn::Lens(lens.id.clone()), lens.label.clone())
                .with_tip("Draw the output as a picture under the command")
        }));
    }
    if state.picture {
        own.push(MenuItem::new(BlockOwn::HidePicture, "Hide").with_tip("Take the picture away"));
    }
    menu_items(Target::Block(state.facts), many, own)
}

/// The menu at `at`.
#[component]
#[allow(non_snake_case)]
pub fn BlockMenu(
    at: BlockMenuAt,
    state: BlockMenuState,
    onpick: EventHandler<Pick<BlockOwn>>,
    onclose: EventHandler<()>,
) -> Element {
    rsx! {
        Menu {
            placement: MenuPlacement::Context,
            anchor: Anchor::Point(at.point),
            items: block_menu_items(state),
            onpick: move |pick: Pick<BlockOwn>| onpick.call(pick),
            onclose: move |()| onclose.call(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(state: BlockMenuState) -> Vec<String> {
        block_menu_items(state)
            .into_iter()
            .filter_map(|item| match item {
                MenuItem::Item { title, .. } => Some(title),
                _ => None,
            })
            .collect()
    }

    fn state(picked: usize, lenses: &[(&str, &str)], picture: bool) -> BlockMenuState {
        BlockMenuState {
            facts: BlockFacts {
                picked,
                has_output: true,
                ..BlockFacts::default()
            },
            lenses: lenses
                .iter()
                .map(|(id, label)| LensOffer {
                    id: (*id).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
            picture,
        }
    }

    #[test]
    fn a_lens_item_for_each_lens_of_one_block_and_hide_only_under_a_picture() {
        let kube = [("kube", "Show with Kube"), ("graph", "Show with Graph")];
        let tail = |state| -> Vec<String> { titles(state).into_iter().skip(6).collect() };
        assert_eq!(
            tail(state(1, &kube, false)),
            ["Show with Kube", "Show with Graph"]
        );
        assert_eq!(
            tail(state(1, &kube, true)),
            ["Show with Kube", "Show with Graph", "Hide"]
        );
        assert_eq!(tail(state(1, &[], true)), ["Hide"]);
        assert!(
            !titles(state(2, &kube, false)).contains(&"Show with Kube".to_owned()),
            "a lens draws one block"
        );
    }
}
