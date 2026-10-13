//! The menu a right-click on a block opens, as Warp's does: copy the picked blocks whole, their
//! commands or their outputs, put the commands back in the prompt, bookmark them, or pick every
//! block. The right-click picks the block first (keeping the pick when the block was in it), so
//! each item acts on what is tinted. Each item runs the action its key runs.

use dioxus::prelude::*;
use ds::host::measure::Anchor;
use ds::prelude::*;
use temor_pane::keys::Action;

/// Where the menu is up: the right-click's point in the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockMenuAt {
    pub point: Point,
}

/// What the menu says, read from the session when it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockMenuState {
    /// How many blocks are picked: one block's items say "Command", several "Commands".
    pub picked: usize,
    /// Whether every picked block is bookmarked: the item then takes the bookmarks away.
    pub bookmarked: bool,
}

/// The menu's items, each the action its key runs.
pub fn block_menu_items(state: BlockMenuState) -> Vec<MenuItem<Action>> {
    let many = state.picked > 1;
    let (commands, outputs) = match many {
        true => ("Copy Commands", "Copy Outputs"),
        false => ("Copy Command", "Copy Output"),
    };
    let bookmark = match state.bookmarked {
        true => "Remove Bookmark",
        false => "Bookmark",
    };
    vec![
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
}

/// The menu at the right-click.
#[component]
#[allow(non_snake_case)]
pub fn BlockMenu(
    at: BlockMenuAt,
    state: BlockMenuState,
    onpick: EventHandler<Action>,
    onclose: EventHandler<()>,
) -> Element {
    rsx! {
        Menu {
            placement: MenuPlacement::Context,
            anchor: Anchor::Point(at.point),
            items: block_menu_items(state),
            onpick: move |action: Action| onpick.call(action),
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

    #[test]
    fn the_items_speak_of_one_block_or_several_and_the_bookmark_turns_around() {
        let one = titles(BlockMenuState {
            picked: 1,
            bookmarked: false,
        });
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
            picked: 3,
            bookmarked: true,
        });
        assert!(many.contains(&"Copy Commands".to_owned()));
        assert!(many.contains(&"Remove Bookmark".to_owned()));
    }
}
