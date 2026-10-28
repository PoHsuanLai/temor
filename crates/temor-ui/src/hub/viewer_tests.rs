//! Viewer panes in the hub: their names, where Esc goes back to, the files the arrows walk, and
//! what a restore does with a file that is gone. No program is started.

use std::path::PathBuf;

use temor_client::model::vocab::ids::PaneId;
use temor_client::model::vocab::text::ViewerRef;
use temor_client::model::workspace::{ViewerOpen, WorkspaceIn, WorkspaceOut, workspace_step};

use super::model::Hub;
use super::tests::hub_after;
use crate::session::ViewAsk;

/// A hub with one tab, a viewer on `file` split from its shell, and the viewer announced.
fn with_viewer(file: &str, around: &[&str]) -> (Hub, PaneId, PaneId) {
    let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
    let shell = PaneId(1);
    let (ws, outs) = workspace_step(
        hub.book.ws.clone(),
        WorkspaceIn::OpenViewer {
            file: ViewerRef(file.to_owned()),
            from: Some(shell),
        },
    );
    hub.book.ws = ws;
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
        site: temor_client::model::shelf::PaneSite::Local,
    });
    hub.viewer_opened(open);
    (hub, shell, pane)
}

#[test]
fn a_page_is_the_rows_stage_not_a_tile_and_names_the_row() {
    let (hub, _, _) = with_viewer("/w/notes/a.txt", &[]);
    let data = hub.sidebar();
    let row = &data.loose[0];
    assert!(row.tiles.is_empty(), "one terminal: no split row");
    assert_eq!(row.title, "a.txt");
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
        hub.book.ws.clone(),
        WorkspaceIn::Split {
            towards: temor_client::model::split::Direction::Down,
        },
    );
    hub.book.ws = ws;
    let (ws, _) = workspace_step(hub.book.ws.clone(), WorkspaceIn::ClosePane(Some(shell)));
    hub.book.ws = ws;
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
        tab: temor_client::model::vocab::ids::TabId(1),
        file: ViewerRef("/nowhere/at/all/a.txt".to_owned()),
        from: None,
    });
    assert!(hub.book.ws.viewers.is_empty());
    assert!(
        hub.notices.iter().any(|n| n.contains("gone")),
        "{:?}",
        hub.notices
    );
}

#[test]
fn a_viewer_of_a_host_file_takes_its_copy_away_with_it_and_never_a_file_of_the_person() {
    let (mut hub, _, viewer) = with_viewer("/w/a.txt", &[]);
    let scratch = tempfile::tempdir().expect("scratch");
    hub.launch.ambient.runtime_dir = scratch.path().join("runtime");
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

#[test]
fn a_viewer_that_only_describes_its_file_says_so_until_one_it_can_show_opens() {
    use anyview_machines::StageFamily::{PeekOnly, Raster, Text};
    let (mut hub, _, viewer) = with_viewer("/w/a.bin", &[]);
    let steps = [
        (PeekOnly, true),
        (Text, false),
        (PeekOnly, true),
        (Raster, false),
    ];
    for (family, want) in steps {
        hub.viewer_family(viewer, family);
        assert_eq!(hub.viewer_describes_only(viewer), want, "{family:?}");
    }
}

#[test]
fn a_file_that_could_not_be_opened_is_offered_to_another_app_until_an_open_succeeds() {
    use anyview_machines::StageFamily::Text;
    let (mut hub, _, viewer) = with_viewer("/w/a.txt", &[]);
    assert_eq!(hub.viewer_failed_file(viewer), None);
    let broken =
        anyview_pane::FilePath::new(std::path::Path::new("/w/broken.pdf")).expect("a path");
    hub.viewer_not_opened(viewer, broken.clone());
    // The failed file, not the one recorded as shown, is the one another app is handed.
    assert_eq!(hub.viewer_failed_file(viewer), Some(&broken));
    assert!(!hub.viewer_describes_only(viewer));
    hub.viewer_family(viewer, Text);
    assert_eq!(hub.viewer_failed_file(viewer), None);
}

#[test]
fn find_in_a_file_is_asked_only_when_a_viewer_has_the_keyboard() {
    use std::{cell::RefCell, rc::Rc};

    use super::model::HubRef;

    // A terminal has the keyboard: ⌘F there is the terminal's own find, so the bar opens plain.
    let hub = HubRef(Rc::new(RefCell::new(hub_after(vec![
        WorkspaceIn::NewTab { cwd: None },
    ]))));
    hub.want_find_in_viewer();
    assert!(!hub.take_finding_wanted());
}
