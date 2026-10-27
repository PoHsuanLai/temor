//! The Favourites at the top of the sidebar (PLAN §7, v14), as Arc's are: square tiles four to a
//! row, each a place the person keeps, read from temor's store (`temor://` URLs, `use_live`). A
//! tab's row dragged here keeps its folder. Pressing a tile brings the tab in that folder to the
//! front, or opens one there; its menu (the one menu's) opens, shows or lets it go. With none kept they take no room, and a drag
//! shows where to drop one over the top of the list, so the rows below never move under it.

use dioxus::prelude::*;
use ds::host::measure::{Anchor, use_rect};
use ds::motion::drag::DragPhase;
use ds::prelude::*;
use temor_client::model::menu::{Item, Target as Clicked};
use temor_client::model::vocab::ids::TabId;
use temor_client::model::vocab::text::Cwd;
use temor_client::model::workspace::WorkspaceIn;
use temor_client::query::OnShelf;
use temor_client::{Command, Shelf};
use temor_schema::url::{HistoryScope, Host, Location, TemorUrl};

use super::dnd::{Dnd, Target};
use super::live::{client, use_live};
use super::menu::{Pick, menu_items};
use super::row_menu::show_in_folder;
use super::row_tip::RowTip;
use super::tab_wiring::{selected_attr, tab_menu_trigger};
use crate::hub::{Desk, TabRow, select_tab};

/// A local folder's URL: what a tab's row keeps when it is dropped here.
pub fn folder_url(place: &str) -> TemorUrl {
    let path = match place.ends_with('/') {
        true => place.to_owned(),
        false => format!("{place}/"),
    };
    TemorUrl::Location(Location {
        host: Host::Local,
        path,
        within: None,
    })
}

/// The local folder a kept place opens, as a tab's folder is written (no trailing `/` but the
/// root's); none for what a tab cannot stand in.
fn folder_of(place: &TemorUrl) -> Option<String> {
    match place {
        TemorUrl::Location(Location {
            host: Host::Local,
            path,
            within: _,
        }) if place_is_folder(path) => Some(match path.trim_end_matches('/') {
            "" => "/".to_owned(),
            trimmed => trimmed.to_owned(),
        }),
        _ => None,
    }
}

fn place_is_folder(path: &str) -> bool {
    path.ends_with('/')
}

/// What a tile is called on hover: its folder's name, `~` for home and `/` for the root.
fn tile_name(folder: &str, home: &str) -> String {
    if folder == home {
        return "~".to_owned();
    }
    folder
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("/")
        .to_owned()
}

/// The Favourites. `rows` are every tab's row, to find the tab a tile stands for.
#[component]
#[allow(non_snake_case)]
pub fn Favourites(rows: Vec<TabRow>, desk: Desk, dnd: Dnd) -> Element {
    let kept = use_live(|| OnShelf(Shelf::Favourites));
    let places: Vec<TemorUrl> = kept
        .read()
        .value()
        .map(|places| places.to_vec())
        .unwrap_or_default();
    let rect = use_rect();
    let drop_rect = use_rect();
    let dragging = matches!(dnd.drag.phase(), DragPhase::Live { .. });
    let empty = places.is_empty();
    // The drag finds the Favourites under the pointer from where they are: the tiles, or with
    // none kept, the box a drag shows over the top of the list.
    dnd.rects.set_favourites(match (empty, dragging) {
        (false, _) => rect.rect(),
        (true, true) => drop_rect.rect(),
        (true, false) => None,
    });
    use_drop({
        let rects = dnd.rects.clone();
        move || rects.set_favourites(None)
    });
    let over = ((dnd.over)() == Some(Target::Favourites)).then_some("true");
    let home = desk.hub.home();
    rsx! {
        div {
            class: "temor-favs",
            role: "group",
            "aria-label": "Favourites",
            "data-over": over,
            "data-empty": empty.then_some("true"),
            onmounted: move |event: MountedEvent| rect.on_mounted(event),
            if empty && dragging {
                div {
                    class: "temor-favs-drop",
                    "data-over": over,
                    onmounted: move |event: MountedEvent| drop_rect.on_mounted(event),
                    "Drop to keep it in Favourites"
                }
            }
            for place in places {
                if let Some(folder) = folder_of(&place) {
                    FavouriteTile {
                        key: "{place}",
                        name: tile_name(&folder, &home),
                        front: rows.iter().any(|row| row.selection == Selection::Selected && row.place.as_deref() == Some(folder.as_str())),
                        tab: rows.iter().find(|row| row.place.as_deref() == Some(folder.as_str())).map(|row| row.id),
                        folder,
                        place,
                        desk: desk.clone(),
                    }
                }
            }
        }
    }
}

/// One kept folder's tile; a secondary press opens its menu.
#[component]
#[allow(non_snake_case)]
fn FavouriteTile(
    place: TemorUrl,
    folder: String,
    name: String,
    front: bool,
    /// The tab already in the folder, if one is.
    tab: Option<TabId>,
    desk: Desk,
) -> Element {
    let rect = use_rect();
    let mut menu = use_signal(|| Shown::Hidden);
    let opening = tab_menu_trigger(move || menu.set(Shown::Visible));
    let selection = match front {
        true => Selection::Selected,
        false => Selection::Unselected,
    };
    let picking = desk.clone();
    let open = folder.clone();
    let (acting, opened) = (desk.clone(), folder.clone());
    let letting = place.clone();
    rsx! {
        RowTip { name: format!("fav-{folder}"), text: folder.clone(),
            div {
                class: "temor-fav",
                role: "button",
                "aria-label": name,
                "data-selected": selected_attr(selection),
                onmounted: move |event: MountedEvent| rect.on_mounted(event),
                onclick: move |_| match tab {
                    Some(tab) => picking.act(select_tab(tab)),
                    None => picking.act(WorkspaceIn::NewTab { cwd: Some(Cwd(open.clone())) }),
                },
                oncontextmenu: opening,
                IconView { source: IconSource::Glyph(Icon::Folder), size: IconSize::Large }
            }
        }
        if let (Shown::Visible, Some(at)) = (menu(), rect.rect()) {
            Menu {
                placement: MenuPlacement::Context,
                anchor: Anchor::Rect(at),
                items: menu_items::<()>(Clicked::Favourite, false, Vec::new()),
                onpick: move |pick: Pick<()>| {
                    let Pick::Item(item) = pick else { return };
                    match item {
                        Item::Open => match tab {
                            Some(tab) => acting.act(select_tab(tab)),
                            None => acting.act(WorkspaceIn::NewTab { cwd: Some(Cwd(opened.clone())) }),
                        },
                        Item::OpenInNewTab => acting.act(WorkspaceIn::NewTab { cwd: Some(Cwd(opened.clone())) }),
                        Item::ShowInFolder => show_in_folder(&acting, std::path::Path::new(&opened)),
                        Item::ShowHistory => acting.open_page(TemorUrl::History(HistoryScope::In(opened.clone()))),
                        Item::RemoveFromFavourites => {
                            client().act(Command::Let { shelf: Shelf::Favourites, place: letting.clone() });
                        }
                        _ => {}
                    }
                },
                onclose: move |()| menu.set(Shown::Hidden),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_kept_as_its_url_and_opened_as_its_path() {
        let url = folder_url("/Users/po/ml");
        assert_eq!(url.to_string(), "temor://local/Users/po/ml/");
        assert_eq!(folder_of(&url).as_deref(), Some("/Users/po/ml"));
        assert_eq!(folder_of(&folder_url("/")).as_deref(), Some("/"));
        assert_eq!(folder_of(&TemorUrl::Downloads), None);
        let file = TemorUrl::Location(Location {
            host: Host::Local,
            path: "/a/b.txt".to_owned(),
            within: None,
        });
        assert_eq!(folder_of(&file), None, "a file is not a tab's folder");
    }

    #[test]
    fn a_tile_is_named_by_its_folder() {
        assert_eq!(tile_name("/Users/po/ml", "/Users/po"), "ml");
        assert_eq!(tile_name("/Users/po", "/Users/po"), "~");
        assert_eq!(tile_name("/", "/Users/po"), "/");
    }
}
