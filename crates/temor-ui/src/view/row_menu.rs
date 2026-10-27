//! A tab's row's menu, the one menu for a row (`view::menu`): a new tab in its folder, keeping
//! its folder among the Favourites, its pin, showing its folder, and closing one pane of a split
//! or the tab.

use std::path::Path;

use dioxus::prelude::*;
use ds::host::measure::Anchor;
use ds::prelude::*;
use temor_client::model::menu::{Item, RowFacts, Target};
use temor_client::model::vocab::ids::{PaneId, TabId};
use temor_client::model::vocab::text::Cwd;
use temor_client::model::workspace::{TabsIn, WorkspaceIn};
use temor_client::query::OnShelf;
use temor_client::{Command, Shelf};
use temor_schema::url::{HistoryScope, TemorUrl};

use super::favourites::folder_url;
use super::folder_row::{MoveTo, with_move_menu};
use super::live::{client, use_live};
use super::menu::{Pick, menu_items};
use crate::hub::Desk;
use crate::platform::default_file_shell;

/// The menu hung from a tab's row, opened by a secondary press on it. `place` is the tab's folder
/// on this computer; `close` the pane of a split row it was opened from.
#[component]
#[allow(non_snake_case)]
pub fn RowMenu(
    tab: TabId,
    at: Rect,
    pinned: bool,
    place: Option<String>,
    close: Option<PaneId>,
    desk: Desk,
    onclose: EventHandler<()>,
) -> Element {
    let shelf = use_live(|| OnShelf(Shelf::Favourites));
    let url = place.as_deref().map(folder_url);
    let kept = url.as_ref().is_some_and(|url| {
        shelf
            .read()
            .value()
            .is_some_and(|places| places.contains(url))
    });
    let facts = RowFacts {
        pinned,
        kept,
        local_folder: place.is_some(),
        split_pane: close.is_some(),
    };
    let items = menu_items::<MoveTo>(Target::Row(facts), false, Vec::new());
    let items = with_move_menu(items, &desk.hub.sidebar().folders, tab, |to| to);
    rsx! {
        Menu {
            placement: MenuPlacement::Context,
            anchor: Anchor::Rect(at),
            items,
            onpick: move |pick: Pick<MoveTo>| {
                let item = match pick {
                    Pick::Item(item) => item,
                    Pick::Own(MoveTo::Folder(folder)) => {
                        return desk.act(WorkspaceIn::Tabs(TabsIn::MoveToFolder { tab, folder }));
                    }
                    Pick::Own(MoveTo::NewFolder) => return desk.new_folder(None, Some(tab)),
                };
                match item {
                    Item::OpenInNewTab => desk.act(WorkspaceIn::NewTab { cwd: place.clone().map(Cwd) }),
                    Item::Favourite { kept } => {
                        if let Some(url) = url.clone() {
                            client().act(match kept {
                                true => Command::Let { shelf: Shelf::Favourites, place: url },
                                false => Command::Keep { shelf: Shelf::Favourites, place: url, at: None },
                            });
                        }
                    }
                    // A pinned tab opens again as a shell: it runs nothing of its own.
                    Item::Pin { pinned: false } => desk.act(WorkspaceIn::Tabs(TabsIn::pin_as_shell(tab))),
                    Item::Pin { pinned: true } => desk.act(WorkspaceIn::Tabs(TabsIn::Unpin(tab))),
                    Item::ShowInFolder => {
                        if let Some(place) = place.as_deref() {
                            show_in_folder(&desk, Path::new(place));
                        }
                    }
                    // The row's session, or before its shell has said where it is, its folder.
                    Item::ShowHistory => {
                        let scope = match (desk.hub.session_of_tab(tab), place.clone()) {
                            (Some(id), _) => Some(HistoryScope::Session(id)),
                            (None, Some(folder)) => Some(HistoryScope::In(folder)),
                            (None, None) => None,
                        };
                        if let Some(scope) = scope {
                            desk.open_page(TemorUrl::History(scope));
                        }
                    }
                    Item::ClosePane => desk.act(WorkspaceIn::ClosePane(close)),
                    Item::CloseTab => desk.act(WorkspaceIn::CloseTab(tab)),
                    _ => {}
                }
            },
            onclose,
        }
    }
}

/// Shows `path` in the file manager, or says why it could not.
pub fn show_in_folder(desk: &Desk, path: &Path) {
    if let Err(err) = default_file_shell().reveal(path) {
        desk.say(format!("Could not show {}: {err}", path.display()));
    }
}
