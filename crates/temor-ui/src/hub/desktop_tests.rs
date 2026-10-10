//! What the window shows the desktop services, and how a request from them maps. No panes are
//! started: the workspace is stepped directly, as in `tests`.

use std::path::PathBuf;

use temor_core::block::Outcome;
use temor_core::split::Direction;
use temor_core::vocab::ids::{ExitCode, TabId};
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceIn;
use temor_desktop::{BadgeCount, Exit, Focus, LineRange, PaneRef, PortRefusal, Request};

use super::desktop::{exit_of, new_tab_for};
use super::tests::hub_after;
use crate::edge::desktop_port::Act;

#[test]
fn the_snapshot_lists_panes_in_tab_order_and_counts_unread_tabs_for_the_dock() {
    let mut hub = hub_after(vec![
        WorkspaceIn::NewTab { cwd: None },
        WorkspaceIn::Split {
            towards: Direction::Right,
        },
        WorkspaceIn::NewTab {
            cwd: Some(Cwd("/home/someone/src".to_owned())),
        },
    ]);
    hub.ws.tabs.unread.insert(TabId(1));

    let (snapshot, badge) = hub.desktop_snapshot();

    let ids: Vec<u64> = snapshot.panes.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, vec![1, 2, 3]);
    assert_eq!(snapshot.context.pane_count, 3);
    // The tab in front has the keyboard; the others are in the background.
    let focus: Vec<Focus> = snapshot.panes.iter().map(|p| p.focus).collect();
    assert_eq!(
        focus,
        vec![Focus::Background, Focus::Background, Focus::Focused]
    );
    assert_eq!(
        snapshot.panes[2].cwd,
        Some(PathBuf::from("/home/someone/src"))
    );
    assert_eq!(snapshot.commands.len(), 3);
    assert_eq!(badge, BadgeCount(1));
}

#[test]
fn a_later_launch_asks_for_a_tab_and_a_new_window_is_a_tab_too() {
    let cases = [
        (Request::NewWindow, WorkspaceIn::NewTab { cwd: None }),
        (
            Request::NewTab { cwd: None },
            WorkspaceIn::NewTab { cwd: None },
        ),
        (
            Request::NewTab {
                cwd: Some(PathBuf::from("/tmp/work")),
            },
            WorkspaceIn::NewTab {
                cwd: Some(Cwd("/tmp/work".to_owned())),
            },
        ),
    ];
    for (request, expected) in cases {
        assert_eq!(new_tab_for(request), expected);
    }
}

#[test]
fn a_finished_command_reports_its_status_and_an_unreported_one_does_not_claim_success() {
    let cases = [
        (Outcome::Running, Exit::Running),
        (Outcome::Succeeded, Exit::Code(0)),
        (Outcome::Failed(ExitCode(2)), Exit::Code(2)),
        (Outcome::Stopped(ExitCode(130)), Exit::Code(130)),
        (Outcome::Unreported, Exit::Unknown),
    ];
    for (outcome, expected) in cases {
        assert_eq!(exit_of(outcome), expected);
    }
}

#[test]
fn requests_for_a_pane_that_is_not_there_or_a_line_that_is_not_one_line_are_refused() {
    let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
    let gone = PaneRef(99);
    let range = LineRange { from: 0, count: 5 };
    let refused = [
        Act::ReadPane(gone, range),
        Act::Close(gone),
        Act::Run(gone, "ls".to_owned()),
    ];
    for act in refused {
        assert!(matches!(hub.perform(act), Err(PortRefusal::NoSuchPane)));
    }
    // The pane exists (it has no shell here), and the line is refused before anything is typed.
    let two_lines = Act::Run(PaneRef(1), "ls\nrm -rf x".to_owned());
    assert!(matches!(
        hub.perform(two_lines),
        Err(PortRefusal::Failed(_))
    ));
}
