//! The one menu (PLAN §7, v14): whatever is right-clicked, its menu is built from the same items
//! in the same five groups, in the same order, so the same thing is always in the same place.
//! Open; what this thing itself does; keeping it; showing where it is; and last, what takes it
//! away. Items that do not apply are left out, and so is an empty group. Pure: the window says
//! what was clicked and draws what comes back.

/// What was right-clicked, and what about it changes its menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A tab's row in the sidebar.
    Row(RowFacts),
    /// A tile among the Favourites.
    Favourite,
    /// The blocks picked in a console.
    Block(BlockFacts),
    /// The page on a row's stage. `file` is whether it is a file or a folder on this computer
    /// (a folder's page is), which another app opens and a folder holds; temor's history and
    /// downloads are neither.
    Page { file: bool },
    /// A file on the Downloads page.
    Download(DownloadFacts),
    /// A folder of tabs in the sidebar.
    Folder,
}

/// What a tab's row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowFacts {
    pub pinned: bool,
    /// Whether its folder is among the Favourites.
    pub kept: bool,
    /// Whether it is in a folder on this computer (a tab on another computer is not).
    pub local_folder: bool,
    /// Whether the menu was opened from one pane of a split row: it then offers to close that
    /// pane.
    pub split_pane: bool,
}

/// What a file on the Downloads page is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DownloadFacts {
    /// Whether it has arrived whole, so it can be opened and shown.
    pub ready: bool,
    /// Whether the tab it came from is still open.
    pub from_tab: bool,
    /// Whether it can be taken off the list (one still arriving cannot).
    pub clearable: bool,
}

/// What the picked blocks are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockFacts {
    /// How many are picked; one block's items speak of one.
    pub picked: usize,
    /// Whether the block's command is still running.
    pub running: bool,
    pub has_output: bool,
    /// Whether every picked block is bookmarked.
    pub bookmarked: bool,
    /// Whether every picked block's output is folded.
    pub folded: bool,
    /// Whether the running program can move to a pane of its own.
    pub detachable: bool,
    /// Whether the console is a session on another computer.
    pub on_host: bool,
}

/// One item of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    Open,
    OpenInNewTab,
    /// Move the page out to a small window of its own.
    PopOut,
    OpenInDefaultApp,
    PutInPrompt,
    /// Bookmark, or take the bookmarks away when they are all bookmarked.
    Bookmark {
        on: bool,
    },
    /// Fold the output, or unfold it when it is all folded.
    Fold {
        folded: bool,
    },
    Detach,
    Stop,
    SaveOutput,
    ViewFiles,
    EditLocally,
    SelectAllBlocks,
    /// Add to the Favourites, or take it out when it is kept.
    Favourite {
        kept: bool,
    },
    /// Pin, or unpin when pinned.
    Pin {
        pinned: bool,
    },
    ShowInFolder,
    /// The history page, of the row's session or of the folder.
    ShowHistory,
    /// The tab a download came from.
    ShowTab,
    /// Put the tab in a folder, or take it out of one.
    MoveToFolder,
    /// A folder made in the folder.
    NewFolderInside,
    /// Call the folder something else, where it stands.
    RenameFolder,
    /// Take the folder away; what it held goes to the folder it was in.
    DeleteFolder,
    ClosePane,
    CloseTab,
    ClosePage,
    RemoveFromFavourites,
    /// Take a download off the list; the file stays.
    RemoveFromList,
}

/// The groups, in order; an item is in exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Open,
    Own,
    Keep,
    Show,
    TakeAway,
}

impl Item {
    /// The group the item is in.
    pub fn group(self) -> Group {
        match self {
            Item::Open | Item::OpenInNewTab | Item::PopOut | Item::OpenInDefaultApp => Group::Open,
            Item::PutInPrompt
            | Item::Bookmark { .. }
            | Item::Fold { .. }
            | Item::Detach
            | Item::Stop
            | Item::SaveOutput
            | Item::ViewFiles
            | Item::EditLocally
            | Item::SelectAllBlocks
            | Item::NewFolderInside
            | Item::RenameFolder => Group::Own,
            Item::Favourite { .. } | Item::Pin { .. } | Item::MoveToFolder => Group::Keep,
            Item::ShowInFolder | Item::ShowHistory | Item::ShowTab => Group::Show,
            Item::ClosePane
            | Item::CloseTab
            | Item::ClosePage
            | Item::RemoveFromFavourites
            | Item::RemoveFromList
            | Item::DeleteFolder => Group::TakeAway,
        }
    }

    /// What the item says. `many` is whether it acts on several things.
    pub fn title(self, many: bool) -> &'static str {
        match self {
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
            Item::Detach => "Detach Running Program",
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
}

/// The menu for `target`: its items in groups, in the groups' order, none empty.
pub fn menu_for(target: Target) -> Vec<Vec<Item>> {
    let mut items: Vec<Item> = Vec::new();
    let mut add = |when: bool, item: Item| {
        if when {
            items.push(item);
        }
    };
    match target {
        Target::Row(row) => {
            add(row.local_folder, Item::OpenInNewTab);
            add(row.local_folder, Item::Favourite { kept: row.kept });
            add(true, Item::Pin { pinned: row.pinned });
            // A pinned tab stands above every folder.
            add(!row.pinned, Item::MoveToFolder);
            add(row.local_folder, Item::ShowInFolder);
            add(true, Item::ShowHistory);
            add(row.split_pane, Item::ClosePane);
            add(true, Item::CloseTab);
        }
        Target::Favourite => {
            add(true, Item::Open);
            add(true, Item::OpenInNewTab);
            add(true, Item::ShowInFolder);
            add(true, Item::ShowHistory);
            add(true, Item::RemoveFromFavourites);
        }
        Target::Block(block) => {
            let one = block.picked <= 1;
            let done = !block.running;
            add(done, Item::PutInPrompt);
            add(
                true,
                Item::Bookmark {
                    on: block.bookmarked,
                },
            );
            add(
                block.has_output,
                Item::Fold {
                    folded: block.folded,
                },
            );
            add(block.running && block.detachable, Item::Detach);
            add(block.running, Item::Stop);
            add(one && done && block.has_output, Item::SaveOutput);
            add(one && done && block.has_output, Item::ViewFiles);
            add(block.on_host, Item::EditLocally);
            add(true, Item::SelectAllBlocks);
        }
        Target::Page { file } => {
            add(file, Item::PopOut);
            add(file, Item::OpenInDefaultApp);
            add(file, Item::ShowInFolder);
            add(true, Item::ClosePage);
        }
        Target::Download(download) => {
            add(download.ready, Item::Open);
            add(download.ready, Item::ShowInFolder);
            add(download.from_tab, Item::ShowTab);
            add(download.clearable, Item::RemoveFromList);
        }
        Target::Folder => {
            add(true, Item::NewFolderInside);
            add(true, Item::RenameFolder);
            add(true, Item::DeleteFolder);
        }
    }
    // Each item lands in its group, whatever order it was added in.
    items.sort_by_key(|item| item.group());
    let mut groups: Vec<Vec<Item>> = Vec::new();
    for item in items {
        match groups.last_mut() {
            Some(last) if last[0].group() == item.group() => last.push(item),
            _ => groups.push(vec![item]),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(target: Target) -> Vec<Vec<&'static str>> {
        menu_for(target)
            .into_iter()
            .map(|group| group.into_iter().map(|item| item.title(false)).collect())
            .collect()
    }

    #[test]
    fn a_rows_menu_opens_keeps_shows_and_closes_in_that_order() {
        let row = RowFacts {
            local_folder: true,
            ..RowFacts::default()
        };
        assert_eq!(
            titles(Target::Row(row)),
            [
                vec!["Open in New Tab"],
                vec!["Add to Favourites", "Pin Tab", "Move to Folder"],
                vec!["Show in Folder", "Show History"],
                vec!["Close Tab"],
            ]
        );
        let kept = RowFacts {
            pinned: true,
            kept: true,
            split_pane: true,
            ..row
        };
        assert_eq!(
            titles(Target::Row(kept))[1..],
            [
                vec!["Remove from Favourites", "Unpin Tab"],
                vec!["Show in Folder", "Show History"],
                vec!["Close Pane", "Close Tab"],
            ]
        );
    }

    #[test]
    fn a_row_on_another_computer_has_no_folder_here_to_open_keep_or_show() {
        assert_eq!(
            titles(Target::Row(RowFacts::default())),
            [
                vec!["Pin Tab", "Move to Folder"],
                vec!["Show History"],
                vec!["Close Tab"]
            ]
        );
    }

    #[test]
    fn a_folders_menu_makes_one_inside_renames_and_deletes() {
        assert_eq!(
            titles(Target::Folder),
            [
                vec!["New Folder Inside", "Rename Folder"],
                vec!["Delete Folder"]
            ]
        );
    }

    #[test]
    fn a_favourite_opens_shows_and_goes() {
        assert_eq!(
            titles(Target::Favourite),
            [
                vec!["Open", "Open in New Tab"],
                vec!["Show in Folder", "Show History"],
                vec!["Remove from Favourites"],
            ]
        );
    }

    #[test]
    fn a_blocks_menu_follows_whether_it_runs_and_how_many_are_picked() {
        let done = BlockFacts {
            picked: 1,
            has_output: true,
            ..BlockFacts::default()
        };
        assert_eq!(
            titles(Target::Block(done)),
            [vec![
                "Put in Prompt",
                "Bookmark",
                "Fold Output",
                "Save Output",
                "View Files",
                "Select All Commands"
            ]]
        );
        let running = BlockFacts {
            running: true,
            detachable: true,
            ..done
        };
        assert_eq!(
            titles(Target::Block(running)),
            [vec![
                "Bookmark",
                "Fold Output",
                "Detach Running Program",
                "Stop",
                "Select All Commands"
            ]]
        );
        let many = BlockFacts {
            picked: 3,
            bookmarked: true,
            on_host: true,
            ..done
        };
        let items = &menu_for(Target::Block(many))[0];
        assert!(
            !items.contains(&Item::SaveOutput),
            "one output is saved at a time"
        );
        assert!(items.contains(&Item::Bookmark { on: true }));
        assert!(items.contains(&Item::EditLocally));
    }

    #[test]
    fn temors_own_page_only_closes() {
        assert_eq!(titles(Target::Page { file: false }), [vec!["Close Page"]]);
        assert_eq!(
            titles(Target::Page { file: true })[0],
            ["Pop Out", "Open in Default App"]
        );
    }

    #[test]
    fn a_download_opens_shows_and_comes_off_the_list_once_it_has_arrived() {
        let arrived = DownloadFacts {
            ready: true,
            from_tab: true,
            clearable: true,
        };
        assert_eq!(
            titles(Target::Download(arrived)),
            [
                vec!["Open"],
                vec!["Show in Folder", "Show Tab"],
                vec!["Remove from List"],
            ]
        );
        assert_eq!(
            titles(Target::Download(DownloadFacts::default())),
            Vec::<Vec<&str>>::new()
        );
    }

    #[test]
    fn every_menu_keeps_the_groups_in_order_and_none_is_empty() {
        let targets = [
            Target::Row(RowFacts::default()),
            Target::Row(RowFacts {
                pinned: true,
                kept: true,
                local_folder: true,
                split_pane: true,
            }),
            Target::Favourite,
            Target::Block(BlockFacts::default()),
            Target::Page { file: true },
            Target::Page { file: false },
            Target::Download(DownloadFacts {
                ready: true,
                from_tab: true,
                clearable: true,
            }),
        ];
        for target in targets {
            let groups = menu_for(target);
            assert!(groups.iter().all(|group| !group.is_empty()));
            let order: Vec<Group> = groups.iter().map(|group| group[0].group()).collect();
            let mut sorted = order.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(order, sorted, "{target:?}");
        }
    }
}
