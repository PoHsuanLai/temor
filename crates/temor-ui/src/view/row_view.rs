//! The row in front (PLAN §7, v14): its stage and its console. The console is the row's terminal
//! (or Arc's Split View of two); the stage is the page a command asked for, beside it, under a bar
//! that names the page and holds the Console button, the page's menu and Close. What the row shows of the two is
//! its layout (⌥⌘1 to ⌥⌘3); a row with no page is its console alone.

use std::path::Path;
use std::time::Instant;

use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use ds::components::chrome::split_view::model::{PaneSpec, Share, SplitAxis, SplitPane};
use ds::components::chrome::split_view::view::SplitView;
use ds::host::measure::use_rect;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_client::model::menu::{Item, Target};
use temor_client::model::split::{Closed, SplitTree};
use temor_client::model::stage::Layout;
use temor_client::model::vocab::ids::{PaneId, TabId};
use temor_client::model::workspace::WorkspaceIn;
use temor_panes::pane::keys::Action;
use temor_schema::url::{HistoryScope, TemorUrl};

use super::folder_page::FolderPage;
use super::history_page::HistoryPage;
use super::menu::{Pick, menu_items};
use super::row_menu::show_in_folder;
use super::shelf::DownloadsPage;
use super::tool_button::ToolButton;
use super::tree_view::TreeView;
use super::viewer_pane::{PageKeys, ViewerView};
use crate::hub::{Desk, page_path, temor_page, temor_page_name};

/// The page's share of the row when it first shows beside the console.
const STAGE_SHARE: f32 = 0.62;

/// The row `tab`, whose panes are `tree`; `focus` has the keys.
#[component]
#[allow(non_snake_case)]
pub fn RowView(
    tab: TabId,
    tree: SplitTree,
    focus: PaneId,
    desk: Desk,
    started: Instant,
) -> Element {
    // The layout is the hub's, not a prop: a change to it alone redraws the row.
    let _ = (desk.ui)();
    let page = tree
        .panes()
        .into_iter()
        .find(|pane| desk.hub.is_viewer(*pane));
    let Some(page) = page.filter(|_| tree.zoomed().is_none()) else {
        return rsx! {
            TreeView { tree, focus, desk, started }
        };
    };
    let console = match tree.remove(page) {
        Ok(Closed::Remaining(rest)) => Some(rest),
        _ => None,
    };
    let layout = match console {
        Some(_) => desk.hub.stage_view(tab).layout,
        // A page with no terminal beside it is all there is to show.
        None => Layout::Stage,
    };
    let stage = rsx! {
        Stage { key: "{page.0}", page, layout, desk: desk.clone() }
    };
    let console = console.map(|rest| {
        rsx! {
            TreeView { tree: rest, focus, desk: desk.clone(), started }
        }
    });
    match (layout, console) {
        (Layout::Both, Some(console)) => rsx! {
            SplitView {
                label: "Page and Console",
                panes: vec![SplitPane::new(PaneSpec::sharing(Share(STAGE_SHARE)), stage)],
                axis: SplitAxis::Row,
                {console}
            }
        },
        (Layout::Console, Some(console)) => console,
        _ => stage,
    }
}

/// The stage: the page's bar over the page.
#[component]
#[allow(non_snake_case)]
fn Stage(page: PaneId, layout: Layout, desk: Desk) -> Element {
    let _ = (desk.ui)();
    let shown = desk.hub.page_file(page);
    let own = shown.as_deref().and_then(temor_page);
    let (name, folder) = match &own {
        Some(own) => (temor_page_name(own), page_scope(own, &desk.hub.home())),
        None => shown.as_deref().map(name_and_folder).unwrap_or_default(),
    };
    let console_tip = desk.hub.tooltip(
        match layout.shows_console() {
            true => "Hide the console",
            false => "Show the console",
        },
        Action::ShowStageAndConsole,
    );
    let (toggling, closing, acting) = (desk.clone(), desk.clone(), desk.clone());
    let more = use_rect();
    let mut menu = use_signal(|| Shown::Hidden);
    rsx! {
        div { class: "temor-stage",
            div { class: "temor-page-bar",
                div { class: "temor-page-title", title: shown.clone().unwrap_or_default(),
                    span { class: "temor-page-name", "{name}" }
                    span { class: "temor-page-folder", "{folder}" }
                }
                span {
                    class: "temor-page-tool",
                    "data-pressed": layout.shows_console().then_some("true"),
                    ToolButton {
                        icon: Icon::PanelLeft,
                        size: ControlSize::Small,
                        label: "Console",
                        tip: console_tip,
                        onclick: move |()| toggling.toggle_console(),
                    }
                }
                div {
                    class: "temor-page-tool",
                    onmounted: move |event: MountedEvent| more.on_mounted(event),
                    ToolButton {
                        icon: Icon::Ellipsis,
                        size: ControlSize::Small,
                        label: "More",
                        tip: "What can be done with this page",
                        onclick: move |()| menu.set(Shown::Visible),
                    }
                }
                ToolButton {
                    icon: Icon::X,
                    size: ControlSize::Small,
                    label: "Close Page",
                    tip: "Close this page",
                    onclick: move |()| closing.act(WorkspaceIn::ClosePane(Some(page))),
                }
            }
            div { class: "temor-page",
                match own.clone() {
                    Some(TemorUrl::History(scope)) => rsx! {
                        PageKeys { pane: page, desk: desk.clone(), label: "History",
                            HistoryPage { key: "{scope:?}", scope, desk: desk.clone() }
                        }
                    },
                    Some(TemorUrl::Downloads) => rsx! {
                        PageKeys { pane: page, desk: desk.clone(), label: "Downloads",
                            DownloadsPage { desk: desk.clone() }
                        }
                    },
                    Some(TemorUrl::Location(place)) => rsx! {
                        PageKeys { pane: page, desk: desk.clone(), label: "Folder",
                            FolderPage { key: "{place.path}", folder: place.path, desk: desk.clone() }
                        }
                    },
                    None => rsx! {
                        ViewerView { pane: page, desk: desk.clone() }
                    },
                }
            }
            if let (Shown::Visible, Some(anchor)) = (menu(), more.anchor()) {
                Menu {
                    placement: MenuPlacement::Popup,
                    anchor,
                    items: menu_items::<()>(Target::Page { file: !matches!(own, Some(TemorUrl::History(_) | TemorUrl::Downloads)) }, false, Vec::new()),
                    onpick: move |pick: Pick<()>| match pick {
                        Pick::Item(Item::OpenInDefaultApp) => acting.viewer_open_in_app(page),
                        Pick::Item(Item::ShowInFolder) => {
                            if let Some(file) = acting.hub.page_file(page) {
                                show_in_folder(&acting, &page_path(&file));
                            }
                        }
                        Pick::Item(Item::ClosePage) => acting.act(WorkspaceIn::ClosePane(Some(page))),
                        _ => {}
                    },
                    onclose: move |()| menu.set(Shown::Hidden),
                }
            }
        }
    }
}

/// What a mouse's side button asks of the row in front: Back (`true`), Forward, or nothing.
pub fn walk_of(button: Option<MouseButton>) -> Option<bool> {
    match button? {
        MouseButton::Fourth => Some(true),
        MouseButton::Fifth => Some(false),
        _ => None,
    }
}

/// What of temor's page `page` is shown, under its name: a folder's history names the folder, a
/// folder's page the folder it is in.
fn page_scope(page: &TemorUrl, home: &str) -> String {
    match page {
        TemorUrl::History(HistoryScope::In(folder)) => format!("in {}", tidy(folder, home)),
        TemorUrl::Location(place) => Path::new(place.path.trim_end_matches('/'))
            .parent()
            .map(|parent| tidy(&parent.to_string_lossy(), home))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// `path` with the home folder written as `~`.
fn tidy(path: &str, home: &str) -> String {
    match path.strip_prefix(home) {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("~{rest}")
        }
        _ => path.to_owned(),
    }
}

/// A page's file name, and the folder it is in.
fn name_and_folder(file: &str) -> (String, String) {
    let path = Path::new(file);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.to_owned());
    let folder = path
        .parent()
        .map(|folder| folder.to_string_lossy().into_owned())
        .unwrap_or_default();
    (name, folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_named_by_its_file_beside_its_folder() {
        assert_eq!(
            name_and_folder("/Users/po/ml/runs/loss.png"),
            ("loss.png".to_owned(), "/Users/po/ml/runs".to_owned())
        );
        assert_eq!(name_and_folder("/"), ("/".to_owned(), String::new()));
    }

    #[test]
    fn a_folders_page_is_named_by_the_folder_it_is_in() {
        let page = crate::hub::folder_url("/Users/po/ml/runs");
        assert_eq!(page_scope(&page, "/Users/po"), "~/ml");
        assert_eq!(temor_page_name(&page), "runs");
        assert_eq!(page_path(&page.to_string()), Path::new("/Users/po/ml/runs"));
        assert_eq!(page_path("/Users/po/a.txt"), Path::new("/Users/po/a.txt"));
        let history = TemorUrl::History(HistoryScope::In("/Users/po".to_owned()));
        assert_eq!(page_scope(&history, "/Users/po"), "in ~");
    }

    #[test]
    fn the_mouse_side_buttons_walk_back_and_forward() {
        assert_eq!(walk_of(Some(MouseButton::Fourth)), Some(true));
        assert_eq!(walk_of(Some(MouseButton::Fifth)), Some(false));
        assert_eq!(walk_of(Some(MouseButton::Primary)), None);
        assert_eq!(walk_of(None), None);
    }
}
