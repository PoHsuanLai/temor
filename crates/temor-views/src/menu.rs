//! The one menu (PLAN §7, v14), drawn: core says what is in it for what was clicked
//! (`temor_client::model::menu`), in its five groups; this draws the groups with quire's menu, a separator
//! between each, and tells the caller which item was picked. The caller does the item: what it
//! means depends on what was clicked.

use ds::prelude::*;
use temor_client::model::menu::{Group, Item, Target, menu_for};

/// What a menu's pick is: an item of the one menu, or one the caller added (a lens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick<T> {
    Item(Item),
    Own(T),
}

/// The menu for `target`, its groups parted by separators; `many` is whether it acts on several
/// things. `own` are the caller's items, put at the end of the thing's own group (the second).
pub fn menu_items<T: Clone>(
    target: Target,
    many: bool,
    own: Vec<MenuItem<T>>,
) -> Vec<MenuItem<Pick<T>>> {
    let groups = menu_for(target);
    let up_to_own = groups
        .iter()
        .take_while(|group| group[0].group() <= Group::Own)
        .count();
    let mut parts: Vec<Vec<MenuItem<Pick<T>>>> = groups
        .iter()
        .map(|group| group.iter().map(|item| item_of(*item, many)).collect())
        .collect();
    let own: Vec<MenuItem<Pick<T>>> = own.into_iter().map(|item| item.map(&Pick::Own)).collect();
    if !own.is_empty() {
        match up_to_own {
            0 => parts.insert(0, own),
            n => parts[n - 1].extend(own),
        }
    }
    let mut items = Vec::new();
    for (at, part) in parts.into_iter().enumerate() {
        if at > 0 {
            items.push(MenuItem::Separator);
        }
        items.extend(part);
    }
    items
}

fn item_of<T>(item: Item, many: bool) -> MenuItem<Pick<T>> {
    let row = MenuItem::new(Pick::Item(item), title_of(item, many));
    match tip_of(item) {
        Some(tip) => row.with_tip(tip),
        None => row,
    }
}

/// What `item` says. `many` is whether it acts on several things.
fn title_of(item: Item, many: bool) -> &'static str {
    match item {
        Item::Open => "Open",
        Item::OpenInNewTab => "Open in New Tab",
        Item::PopOut => "Pop Out",
        Item::OpenInDefaultApp => "Open in Default App",
        Item::PutInPrompt => "Put in Prompt",
        Item::Bookmark { on: true } => "Remove Bookmark",
        Item::Bookmark { on: false } => "Bookmark",
        Item::Fold { folded: true } => "Unfold Output",
        Item::Fold { folded: false } => match many {
            true => "Fold Outputs",
            false => "Fold Output",
        },
        Item::Stop => "Stop",
        Item::SaveOutput => "Save Output",
        Item::ViewFiles => "View Files",
        Item::EditLocally => "Edit Locally",
        Item::SelectAllBlocks => "Select All Commands",
        Item::Favourite { kept: true } => "Remove from Favourites",
        Item::Favourite { kept: false } => "Add to Favourites",
        Item::Pin { pinned: true } => "Unpin Tab",
        Item::Pin { pinned: false } => "Pin Tab",
        Item::ShowInFolder => "Show in Folder",
        Item::ShowHistory => "Show History",
        Item::ShowTab => "Show Tab",
        Item::MoveToFolder => "Move to Folder",
        Item::NewFolderInside => "New Folder Inside",
        Item::RenameFolder => "Rename Folder",
        Item::DeleteFolder => "Delete Folder",
        Item::ClosePane => "Close Pane",
        Item::CloseTab => "Close Tab",
        Item::ClosePage => "Close Page",
        Item::RemoveFromFavourites => "Remove from Favourites",
        Item::RemoveFromList => "Remove from List",
    }
}

/// What an item does, said on hover where its words do not say it all.
fn tip_of(item: Item) -> Option<&'static str> {
    Some(match item {
        Item::OpenInNewTab => "Open a new tab in this folder",
        Item::PutInPrompt => "Type the command into the prompt. Press Return to run it",
        Item::Bookmark { on: false } => "Alt+Up and Alt+Down step between bookmarks",
        Item::Fold { .. } => "Show only the command's card",
        Item::Stop => "Interrupt the program (Control-C)",
        Item::SaveOutput => "Save this command's output to Downloads",
        Item::ViewFiles => "Show the files this command listed",
        Item::EditLocally => {
            "Open the selected file, or the last file in the command, in an editor here"
        }
        Item::ShowInFolder => "Show it in your files",
        Item::ShowHistory => "The commands run here, by day",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use temor_client::model::menu::{BlockFacts, RowFacts};

    use super::*;

    /// The menu as its rows: a title, or `-` for a separator.
    fn rows(items: Vec<MenuItem<Pick<&'static str>>>) -> Vec<String> {
        items
            .into_iter()
            .map(|item| match item {
                MenuItem::Item { title, .. } => title,
                _ => "-".to_owned(),
            })
            .collect()
    }

    #[test]
    fn the_groups_are_parted_by_separators() {
        let row = RowFacts {
            local_folder: true,
            ..RowFacts::default()
        };
        assert_eq!(
            rows(menu_items(Target::Row(row), false, Vec::new())),
            [
                "Open in New Tab",
                "-",
                "Add to Favourites",
                "Pin Tab",
                "Move to Folder",
                "-",
                "Show in Folder",
                "Show History",
                "-",
                "Close Tab"
            ]
        );
    }

    #[test]
    fn the_callers_own_items_end_the_things_own_group() {
        let block = BlockFacts {
            picked: 1,
            ..BlockFacts::default()
        };
        let lens = vec![MenuItem::new("kube", "Show with Kube")];
        let shown = rows(menu_items(Target::Block(block), false, lens));
        assert_eq!(shown.last().map(String::as_str), Some("Show with Kube"));
        let favourite = rows(menu_items(
            Target::Favourite,
            false,
            vec![MenuItem::new("x", "Mine")],
        ));
        assert_eq!(favourite[..4], ["Open", "Open in New Tab", "Mine", "-"]);
    }
}
