//! The menu a right-click on a block opens, as Warp's does: copy the picked blocks whole, their
//! commands or their outputs, put the commands back in the prompt, bookmark them, or pick every
//! block. The right-click picks the block first (keeping the pick when the block was in it), so
//! each item acts on what is tinted. Each item runs the action its key runs.

use dioxus::prelude::*;
use ds::host::measure::Anchor;
use ds::prelude::*;
use temor_pane::keys::Action;

use crate::plugins::lens::LensOffer;

/// Where the menu is up: the right-click's point in the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockMenuAt {
    pub point: Point,
}

/// Whether the pane is a session on another computer: the menu then offers to edit a file
/// there on this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    ThisComputer,
    OnHost,
}

/// Whether a lens has drawn a picture under the block: the menu then offers to hide it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureShown {
    Yes,
    No,
}

/// What a pick in the menu asks for: a pane action, a lens to draw the block with (its id), or
/// one of the items with no key of their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockPick {
    Do(Action),
    EditLocally,
    Lens(String),
    HidePicture,
}

/// What the menu says, read from the session when it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMenuState {
    /// How many blocks are picked: one block's items say "Command", several "Commands".
    pub picked: usize,
    /// Whether every picked block is bookmarked: the item then takes the bookmarks away.
    pub bookmarked: bool,
    pub pane: PaneKind,
    /// The lenses that can draw the selected block; offered when one block is picked.
    pub lenses: Vec<LensOffer>,
    pub picture: PictureShown,
}

/// The menu's items, each the action its key runs, and on a pane on another computer "Edit
/// Locally": it edits the selected text, else the last path in the block's command.
pub fn block_menu_items(state: BlockMenuState) -> Vec<MenuItem<BlockPick>> {
    let many = state.picked > 1;
    let (commands, outputs) = match many {
        true => ("Copy Commands", "Copy Outputs"),
        false => ("Copy Command", "Copy Output"),
    };
    let bookmark = match state.bookmarked {
        true => "Remove Bookmark",
        false => "Bookmark",
    };
    let mut items = vec![
        MenuItem::new(Action::Copy, "Copy").with_tip("Copy the commands and their output"),
        MenuItem::new(Action::CopyCommands, commands),
        MenuItem::new(Action::CopyOutputs, outputs),
        MenuItem::Separator,
        MenuItem::new(Action::ReinputCommands, "Put in Prompt")
            .with_tip("Type the commands into the prompt. Press Return to run them"),
        MenuItem::new(Action::ToggleBookmark, bookmark)
            .with_tip("Alt+Up and Alt+Down step between bookmarks"),
        MenuItem::Separator,
        MenuItem::new(Action::SelectAllBlocks, "Select All Commands"),
    ]
    .into_iter()
    .map(|item| item.map(&BlockPick::Do))
    .collect::<Vec<_>>();
    if !state.lenses.is_empty() && !many {
        items.push(MenuItem::Separator);
        items.extend(state.lenses.iter().map(|lens| {
            MenuItem::new(BlockPick::Lens(lens.id.clone()), lens.label.clone())
                .with_tip("Draw the output as a picture under the command")
        }));
    }
    if state.picture == PictureShown::Yes {
        items.push(MenuItem::new(BlockPick::HidePicture, "Hide").with_tip("Take the picture away"));
    }
    if state.pane == PaneKind::OnHost {
        items.push(MenuItem::Separator);
        items.push(
            MenuItem::new(BlockPick::EditLocally, "Edit Locally").with_tip(
                "Open the selected file, or the last file in the command, in an editor here",
            ),
        );
    }
    items
}

/// The menu at the right-click.
#[component]
#[allow(non_snake_case)]
pub fn BlockMenu(
    at: BlockMenuAt,
    state: BlockMenuState,
    onpick: EventHandler<BlockPick>,
    onclose: EventHandler<()>,
) -> Element {
    rsx! {
        Menu {
            placement: MenuPlacement::Context,
            anchor: Anchor::Point(at.point),
            items: block_menu_items(state),
            onpick: move |pick: BlockPick| onpick.call(pick),
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

    fn state(picked: usize, lenses: &[(&str, &str)], picture: PictureShown) -> BlockMenuState {
        BlockMenuState {
            picked,
            bookmarked: false,
            pane: PaneKind::ThisComputer,
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
    fn the_items_speak_of_one_block_or_several_and_the_bookmark_turns_around() {
        let one = titles(state(1, &[], PictureShown::No));
        assert_eq!(
            one,
            [
                "Copy",
                "Copy Command",
                "Copy Output",
                "Put in Prompt",
                "Bookmark",
                "Select All Commands"
            ]
        );
        let many = titles(BlockMenuState {
            bookmarked: true,
            pane: PaneKind::OnHost,
            ..state(3, &[], PictureShown::No)
        });
        assert!(many.contains(&"Copy Commands".to_owned()));
        assert!(many.contains(&"Remove Bookmark".to_owned()));
        // Only a pane on another computer offers to edit a file here.
        assert_eq!(many.last().map(String::as_str), Some("Edit Locally"));
        assert!(!one.contains(&"Edit Locally".to_owned()));
    }

    #[test]
    fn a_lens_item_for_each_lens_of_one_block_and_hide_only_under_a_picture() {
        let kube = [("kube", "Show with Kube"), ("graph", "Show with Graph")];
        let tail = |state| -> Vec<String> { titles(state).into_iter().skip(6).collect() };
        assert_eq!(
            tail(state(1, &kube, PictureShown::No)),
            ["Show with Kube", "Show with Graph"]
        );
        assert_eq!(
            tail(state(1, &kube, PictureShown::Yes)),
            ["Show with Kube", "Show with Graph", "Hide"]
        );
        assert_eq!(tail(state(1, &[], PictureShown::Yes)), ["Hide"]);
        assert!(
            tail(state(2, &kube, PictureShown::No)).is_empty(),
            "a lens draws one block"
        );
        let picks: Vec<BlockPick> = block_menu_items(state(1, &kube, PictureShown::Yes))
            .into_iter()
            .filter_map(|item| match item {
                MenuItem::Item { value, .. } => Some(value),
                _ => None,
            })
            .skip(6)
            .collect();
        assert_eq!(
            picks,
            [
                BlockPick::Lens("kube".to_owned()),
                BlockPick::Lens("graph".to_owned()),
                BlockPick::HidePicture
            ]
        );
    }
}
