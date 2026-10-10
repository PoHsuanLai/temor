//! Viewer panes in the hub: their names, where Esc goes back to, the files the arrows walk, and
//! what a restore does with a file that is gone. No program is started.

use std::path::PathBuf;

use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::ViewerRef;
use temor_core::workspace::{ViewerOpen, WorkspaceIn, WorkspaceOut, workspace_step};

use super::model::Hub;
use super::tests::hub_after;
use crate::session::ViewAsk;

/// A hub with one tab, a viewer on `file` split from its shell, and the viewer announced.
fn with_viewer(file: &str, around: &[&str]) -> (Hub, PaneId, PaneId) {
    let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
    let shell = PaneId(1);
    let (ws, outs) = workspace_step(
        hub.ws.clone(),
        WorkspaceIn::OpenViewer {
            file: ViewerRef(file.to_owned()),
            from: Some(shell),
        },
    );
    hub.ws = ws;
    let open = outs
        .into_iter()
        .find_map(|out| match out {
            WorkspaceOut::Viewer(open) => Some(open),
            _ => None,
        })
        .expect("the workspace announces the viewer");
    let pane = open.pane;
    hub.viewers_ask(ViewAsk {
        file: PathBuf::from(file),
        around: around.iter().map(PathBuf::from).collect(),
        site: temor_core::shelf::PaneSite::Local,
    });
    hub.viewer_opened(open);
    (hub, shell, pane)
}

#[test]
fn a_viewer_tile_is_named_by_its_file() {
    let (hub, _, viewer) = with_viewer("/w/notes/a.txt", &[]);
    let data = hub.sidebar();
    let tiles = &data.loose[0].tiles;
    let name = tiles
        .iter()
        .find(|t| t.pane == viewer)
        .map(|t| t.title.as_str());
    assert_eq!(name, Some("a.txt"));
    assert_eq!(hub.window_parts(), ("a.txt".to_owned(), None));
}

#[test]
fn the_arrows_walk_the_files_around_the_one_opened() {
    let (hub, _, viewer) = with_viewer("/w/b.txt", &["/w/a.txt", "/w/b.txt", "/w/c.txt"]);
    let show = hub.viewer_show(viewer).expect("a viewer to show");
    assert_eq!(show.file.as_path(), std::path::Path::new("/w/b.txt"));
    let sequence = show.sequence.expect("a sequence");
    assert_eq!(sequence.entries().count().get(), 3);
    assert_eq!(
        sequence.current().as_path(),
        std::path::Path::new("/w/b.txt")
    );
}

#[test]
fn a_file_the_output_did_not_list_has_no_sequence_and_a_relative_one_is_not_shown() {
    let (hub, _, viewer) = with_viewer("/w/z.txt", &["/w/a.txt"]);
    assert_eq!(hub.viewer_show(viewer).map(|s| s.sequence), Some(None));
    let (hub, _, viewer) = with_viewer("relative.txt", &[]);
    assert!(hub.viewer_show(viewer).is_none());
}

#[test]
fn escape_goes_back_to_the_terminal_the_file_came_from() {
    let (hub, shell, viewer) = with_viewer("/w/a.txt", &[]);
    assert_eq!(hub.viewer_unfocus(viewer), Some(WorkspaceIn::Focus(shell)));
}

#[test]
fn escape_goes_to_another_terminal_when_the_origin_is_gone() {
    let (mut hub, shell, viewer) = with_viewer("/w/a.txt", &[]);
    let (ws, _) = workspace_step(
        hub.ws.clone(),
        WorkspaceIn::Split {
            towards: temor_core::split::Direction::Down,
        },
    );
    hub.ws = ws;
    let (ws, _) = workspace_step(hub.ws.clone(), WorkspaceIn::ClosePane(Some(shell)));
    hub.ws = ws;
    let back = hub.viewer_unfocus(viewer);
    assert!(
        matches!(back, Some(WorkspaceIn::Focus(pane)) if pane != viewer && pane != shell),
        "{back:?}"
    );
}

#[test]
fn a_restored_viewer_whose_file_is_gone_is_closed_with_a_word_to_the_person() {
    let (mut hub, _, viewer) = with_viewer("/nowhere/at/all/a.txt", &[]);
    hub.restore_viewer(ViewerOpen {
        pane: viewer,
        tab: temor_core::vocab::ids::TabId(1),
        file: ViewerRef("/nowhere/at/all/a.txt".to_owned()),
        from: None,
    });
    assert!(hub.ws.viewers.is_empty());
    assert!(
        hub.notices.iter().any(|n| n.contains("gone")),
        "{:?}",
        hub.notices
    );
}

#[test]
fn a_viewer_of_a_host_file_takes_its_copy_away_with_it_and_never_a_file_of_the_person() {
    let (mut hub, _, viewer) = with_viewer("/w/a.txt", &[]);
    let folder = hub.copies_dir().join("7");
    std::fs::create_dir_all(&folder).expect("copy folder");
    let copy = folder.join("a.txt");
    std::fs::write(&copy, "from the host").expect("copy");
    let theirs = hub
        .copies_dir()
        .parent()
        .expect("runtime")
        .join("theirs.txt");
    std::fs::write(&theirs, "mine").expect("a local file");
    hub.host_copy_shown(viewer, &theirs);
    hub.viewer_ended(viewer);
    assert!(theirs.exists(), "a file that is not a copy is left alone");
    hub.host_copy_shown(viewer, &copy);
    hub.viewer_ended(viewer);
    assert!(
        !copy.exists() && !folder.exists(),
        "the copy and its folder go"
    );
}
