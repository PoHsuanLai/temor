//! A folder of tabs in the sidebar (PLAN §7), as Arc's are: quire's outline `Row` with its tabs
//! under it, then the folders in it, one step in. A press on it opens or closes it, as its
//! triangle does; a tab dropped on it goes in; a right click is its menu (New Folder Inside,
//! Rename Folder, Delete Folder). It is named where it stands, as mailo's folders are: Return
//! keeps the name, Escape the old one.

use dioxus::prelude::*;
use ds::base::press::{PointerButton, Press};
use ds::base::vocab::{Availability, DropState, RowState};
use ds::components::lists::row::row::Outline;
use ds::components::overlays::drag_ghost::DropLine;
use ds::focus::request::use_focus_request;
use ds::host::measure::{Anchor, use_rect};
use ds::prelude::*;
use temor_client::model::menu::{Item, Target as MenuTarget};
use temor_client::model::vocab::ids::{FolderId, TabId};
use temor_client::model::vocab::text::TabTitle;
use temor_client::model::workspace::{TabsIn, WorkspaceIn};

use super::dnd::{Dnd, Target};
use super::menu::{Pick, menu_items};
use super::tab_row::TabRowView;
use super::tab_wiring::tab_menu_trigger;
use crate::hub::{Desk, FolderRows};

/// The row of `folder` and, while it is open, what it holds; `all` are the Space's folders, the
/// ones in it among them.
#[component]
#[allow(non_snake_case)]
pub fn FolderRowView(folder: FolderRows, all: Vec<FolderRows>, desk: Desk, dnd: Dnd) -> Element {
    let id = folder.id;
    let mut menu = use_signal(|| Shown::Hidden);
    let rect = use_rect();
    if let Some(at) = rect.rect() {
        dnd.rects.set_folder(id, at);
    }
    use_drop({
        let rects = dnd.rects.clone();
        move || rects.forget_folder(id)
    });
    let shown = match folder.open {
        true => Shown::Visible,
        false => Shown::Hidden,
    };
    let toggling = desk.clone();
    let opening = desk.clone();
    let open = folder.open;
    let naming = (desk.naming)() == Some(id);
    let edit = naming.then(|| {
        rsx! {
            FolderName { folder: id, title: folder.title.clone(), desk: desk.clone() }
        }
    });
    let drop = match (dnd.over)() {
        Some(Target::Folder(over)) if over == id => DropState::Target,
        _ => DropState::Idle,
    };
    let inside: Vec<FolderRows> = all
        .iter()
        .filter(|other| other.parent == Some(id))
        .cloned()
        .collect();
    rsx! {
        div {
            class: "temor-side-folder",
            oncontextmenu: tab_menu_trigger(move || menu.set(Shown::Visible)),
            Row {
                title: folder.title.as_str(),
                leading: RowLeading::Source(IconSource::Glyph(Icon::Folder)),
                outline: Outline::Branch(shown),
                edit,
                state: RowState {
                    availability: Availability::Enabled,
                    drop,
                    ..RowState::default()
                },
                on_toggle: move |shown: Shown| {
                    let open = shown == Shown::Visible;
                    toggling.act(WorkspaceIn::Tabs(TabsIn::OpenFolder { folder: id, open }));
                },
                onclick: move |press: Press| {
                    if !matches!(press.button, PointerButton::Secondary) {
                        let open = !open;
                        opening.act(WorkspaceIn::Tabs(TabsIn::OpenFolder { folder: id, open }));
                    }
                },
                common: Common {
                    mounted: Some(EventHandler::new(move |event: MountedEvent| rect.on_mounted(event))),
                    ..Common::default()
                },
                div { class: "temor-side-folder-kids",
                    for row in folder.rows.iter() {
                        TabRowView {
                            key: "{row.id.0}",
                            row: row.clone(),
                            pinned: false,
                            desk: desk.clone(),
                            dnd: dnd.clone(),
                        }
                    }
                    for kid in inside {
                        FolderRowView {
                            key: "f{kid.id.0}",
                            folder: kid,
                            all: all.clone(),
                            desk: desk.clone(),
                            dnd: dnd.clone(),
                        }
                    }
                }
            }
            if drop == DropState::Target {
                DropLine {}
            }
            if let (Shown::Visible, Some(at)) = (menu(), rect.rect()) {
                FolderMenu { folder: id, at, desk: desk.clone(), onclose: move |()| menu.set(Shown::Hidden) }
            }
        }
    }
}

/// A folder's menu: the one menu for a folder.
#[component]
#[allow(non_snake_case)]
fn FolderMenu(folder: FolderId, at: Rect, desk: Desk, onclose: EventHandler<()>) -> Element {
    rsx! {
        Menu {
            placement: MenuPlacement::Context,
            anchor: Anchor::Rect(at),
            items: menu_items::<()>(MenuTarget::Folder, false, Vec::new()),
            onpick: move |pick: Pick<()>| {
                let Pick::Item(item) = pick else { return };
                // A folder being named has the keyboard in its name field instead.
                if !matches!(item, Item::NewFolderInside | Item::RenameFolder) {
                    desk.keyboard_back();
                }
                match item {
                    Item::NewFolderInside => desk.new_folder(Some(folder), None),
                    Item::RenameFolder => {
                        let mut naming = desk.naming;
                        naming.set(Some(folder));
                    }
                    Item::DeleteFolder => desk.act(WorkspaceIn::Tabs(TabsIn::RemoveFolder(folder))),
                    _ => {}
                }
            },
            onclose,
        }
    }
}

/// A folder's name, written where the row says it: quire's `TextField` without its bezel, with
/// the keyboard as it mounts and the old name selected. Return keeps what is typed (an empty name
/// keeps the old one) and Escape leaves it; either ends the naming.
#[component]
#[allow(non_snake_case)]
fn FolderName(folder: FolderId, title: String, desk: Desk) -> Element {
    let mut typed = use_signal(|| title.clone());
    let focus = FieldFocus::Controlled(use_focus_request().with_select_all());
    let mut naming = desk.naming;
    rsx! {
        TextField {
            label: "Folder name".to_owned(),
            value: typed(),
            placeholder: title.clone(),
            bezel: FieldBezel::Plain,
            focus,
            oninput: move |text: String| typed.set(text),
            onkey: move |event: KeyboardEvent| {
                // Every key is the field's: a letter typed into a name is not a shortcut.
                event.stop_propagation();
                match event.key() {
                    Key::Enter => {
                        let name = typed.peek().trim().to_owned();
                        naming.set(None);
                        if !name.is_empty() && name != title {
                            let title = TabTitle(name);
                            desk.act(WorkspaceIn::Tabs(TabsIn::RenameFolder { folder, title }));
                        }
                    }
                    Key::Escape => naming.set(None),
                    _ => {}
                }
            },
        }
    }
}

/// Where a row's "Move to Folder" puts its tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTo {
    /// Into a folder, or (`None`) out of every folder.
    Folder(Option<FolderId>),
    /// Into a folder made for it.
    NewFolder,
}

/// `items` with "Move to Folder" opening its submenu: the Space's `folders`, each one step in
/// under the folder it is in and the one `tab` is in checked, "Out of Folder" when it is in one,
/// and "New Folder".
pub fn with_move_menu<T: Clone>(
    items: Vec<MenuItem<Pick<T>>>,
    folders: &[FolderRows],
    tab: TabId,
    lift: impl Fn(MoveTo) -> T,
) -> Vec<MenuItem<Pick<T>>> {
    let here = folders
        .iter()
        .find(|folder| folder.rows.iter().any(|row| row.id == tab))
        .map(|folder| folder.id);
    let depth = |folder: &FolderRows| {
        let mut depth = 0;
        let mut parent = folder.parent;
        while let Some(id) = parent {
            depth += 1;
            parent = folders.iter().find(|f| f.id == id).and_then(|f| f.parent);
        }
        depth
    };
    let own = |to: MoveTo| Pick::Own(lift(to));
    let mut children: Vec<MenuItem<Pick<T>>> = folders
        .iter()
        .map(|folder| {
            let title = format!("{}{}", "    ".repeat(depth(folder)), folder.title);
            let check = match here == Some(folder.id) {
                true => Check::On,
                false => Check::Off,
            };
            MenuItem::new(own(MoveTo::Folder(Some(folder.id))), title)
                .with_image(MenuImage::Icon(Icon::Folder))
                .with_check(check)
        })
        .collect();
    if here.is_some() {
        children.push(MenuItem::new(own(MoveTo::Folder(None)), "Out of Folder"));
    }
    if !children.is_empty() {
        children.push(MenuItem::Separator);
    }
    children.push(
        MenuItem::new(own(MoveTo::NewFolder), crate::hub::NEW_FOLDER)
            .with_image(MenuImage::Icon(Icon::Plus)),
    );
    items
        .into_iter()
        .map(|item| match item {
            MenuItem::Item {
                value: Pick::Item(Item::MoveToFolder),
                title,
                ..
            } => MenuItem::Submenu {
                title,
                image: None,
                availability: Availability::Enabled,
                children: children.clone(),
            },
            other => other,
        })
        .collect()
}
