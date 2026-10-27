//! A row's stage in the hub: pages opened from its terminal are steps Back and Forward walk, a
//! page shown can be shown alone or hidden behind the console, and the keys follow what is seen.
//! No program is started.

use std::path::PathBuf;

use temor_client::model::stage::Layout;
use temor_client::model::vocab::ids::{PaneId, TabId};
use temor_client::model::workspace::WorkspaceIn;

use super::model::Hub;
use super::tests::hub_after;
use crate::session::ViewAsk;

fn ask(file: &str) -> ViewAsk {
    ViewAsk {
        file: PathBuf::from(file),
        around: Vec::new(),
        site: temor_client::model::shelf::PaneSite::Local,
    }
}

/// A hub with one tab around shell 1.
fn hub() -> (Hub, TabId, PaneId) {
    let hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
    let tab = hub.book.ws.front().expect("a tab");
    (hub, tab, PaneId(1))
}

fn shown(hub: &Hub, tab: TabId) -> Option<String> {
    let pane = hub
        .book
        .ws
        .tabs
        .tab(tab)?
        .tree
        .panes()
        .into_iter()
        .find(|pane| hub.is_viewer(*pane))?;
    hub.book
        .ws
        .viewers
        .get(&pane)
        .map(|file| file.as_str().to_owned())
}

#[test]
fn a_row_with_no_page_shows_its_console_and_has_nowhere_to_go() {
    let (hub, tab, _) = hub();
    let view = hub.stage_view(tab);
    assert_eq!(view.layout, Layout::Console);
    assert!(!view.can_go_back && !view.can_go_forward);
}

#[test]
fn back_and_forward_walk_the_pages_the_row_opened() {
    let (mut hub, tab, shell) = hub();
    hub.open_viewer(shell, ask("/w/a.txt"));
    hub.open_viewer(shell, ask("/w/b.txt"));
    assert_eq!(hub.stage_view(tab).layout, Layout::Both);
    assert_eq!(shown(&hub, tab).as_deref(), Some("/w/b.txt"));
    hub.walk_pages(true);
    assert_eq!(shown(&hub, tab).as_deref(), Some("/w/a.txt"));
    assert!(hub.stage_view(tab).can_go_forward);
    hub.walk_pages(false);
    assert_eq!(shown(&hub, tab).as_deref(), Some("/w/b.txt"));
    assert!(
        !hub.stage_view(tab).can_go_forward,
        "walking is not a new step"
    );
}

#[test]
fn a_closed_page_opens_again_with_back() {
    let (mut hub, tab, shell) = hub();
    hub.open_viewer(shell, ask("/w/a.txt"));
    let viewer = hub.keyboard_pane().expect("the page has the keys");
    hub.act(WorkspaceIn::ClosePane(Some(viewer)));
    assert_eq!(shown(&hub, tab), None);
    assert_eq!(hub.stage_view(tab).layout, Layout::Console);
    hub.walk_pages(true);
    assert_eq!(shown(&hub, tab).as_deref(), Some("/w/a.txt"));
}

#[test]
fn hiding_what_has_the_keys_hands_them_to_what_is_shown() {
    let (mut hub, tab, shell) = hub();
    hub.open_viewer(shell, ask("/w/a.txt"));
    let viewer = hub.keyboard_pane().expect("the page has the keys");
    assert_ne!(viewer, shell);
    hub.set_layout(Layout::Console);
    assert_eq!(hub.stage_view(tab).layout, Layout::Console);
    assert_eq!(hub.keyboard_pane(), Some(shell));
    hub.set_layout(Layout::Stage);
    assert_eq!(hub.keyboard_pane(), Some(viewer));
}

/// The names the address shows, and whether it ends at a file.
fn address(hub: &Hub) -> (Vec<String>, bool) {
    match hub.address() {
        super::Address::Path { crumbs, file } => {
            (crumbs.into_iter().map(|crumb| crumb.name).collect(), file)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_address_is_the_page_shown_or_else_the_terminal_s_folder() {
    let mut hub = hub_after(vec![WorkspaceIn::NewTab {
        cwd: Some(temor_client::model::vocab::text::Cwd("/w/src".to_owned())),
    }]);
    assert_eq!(
        address(&hub),
        (vec!["/".into(), "w".into(), "src".into()], false)
    );
    hub.open_viewer(PaneId(1), ask("/w/docs/a.txt"));
    assert_eq!(
        address(&hub),
        (
            vec!["/".into(), "w".into(), "docs".into(), "a.txt".into()],
            true
        )
    );
    hub.set_layout(Layout::Console);
    assert_eq!(address(&hub).0.last().map(String::as_str), Some("src"));
    hub.set_layout(Layout::Stage);
    hub.open_page(temor_schema::url::TemorUrl::Downloads);
    assert_eq!(
        hub.address(),
        super::Address::Page(temor_schema::url::TemorUrl::Downloads)
    );
}
