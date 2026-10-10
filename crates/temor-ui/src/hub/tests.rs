//! The hub's pure parts: which request a pane action makes, and what the sidebar shows for a
//! workspace. No panes are started; the workspace is built with its step.

use temor_core::split::{Direction, Goto};
use temor_core::vocab::ids::{FolderId, TabId};
use temor_core::vocab::text::{Relaunch, TabTitle};
use temor_core::workspace::{NthTab, TabSel, TabsIn, WorkspaceIn, WorkspaceOut, workspace_step};
use temor_pane::keys::Action;

use super::keys::workspace_input;
use super::model::Hub;
use super::rows::{Unread, tidy};
use crate::edge::Ambient;
use crate::view::launch::{Launch, LauncherChoice};
use crate::view::theme::AppearanceSource;

pub(super) fn launch() -> Launch {
    let home = std::path::PathBuf::from("/home/someone");
    Launch {
        ambient: Ambient {
            home: home.clone(),
            env: temor_shell::ChildEnv::new(),
            config_dir: home.join("config"),
            runtime_dir: home.join("runtime"),
        },
        settings: temor_pane::settings::TerminalSettings::default(),
        keymap: crate::edge::keymap_for(
            chordkit::Platform::Linux {
                desktop: chordkit::Desktop::Ours,
            },
            &chordkit::Overrides::default(),
        )
        .0,
        launcher: LauncherChoice::ThisProgram,
        appearance: AppearanceSource::scratch(
            home.join("config"),
            ds::prelude::SystemPrefs::default(),
        ),
        probe: None,
        state_dir: None,
    }
}

/// A hub whose workspace went through `steps`, with no pane started.
pub(super) fn hub_after(steps: Vec<WorkspaceIn>) -> Hub {
    let mut hub = Hub::new(launch());
    for step in steps {
        let (ws, outs) = workspace_step(hub.ws.clone(), step);
        // The step only describes panes; starting them is the edge's, and these tests have none.
        assert!(
            outs.iter().all(|o| !matches!(o, WorkspaceOut::Refused(_))),
            "{outs:?}"
        );
        hub.ws = ws;
    }
    hub
}

#[test]
fn each_tab_and_split_action_makes_its_request_and_the_others_make_none() {
    let nth = |n| WorkspaceIn::Tabs(TabsIn::Select(TabSel::Nth(NthTab::new(n).expect("1 to 9"))));
    #[rustfmt::skip]
    let rows: &[(Action, Option<WorkspaceIn>)] = &[
        (Action::NewTab, Some(WorkspaceIn::NewTab { cwd: None })),
        (Action::ClosePane, Some(WorkspaceIn::ClosePane(None))),
        (Action::SplitRight, Some(WorkspaceIn::Split { towards: Direction::Right })),
        (Action::SplitDown, Some(WorkspaceIn::Split { towards: Direction::Down })),
        (Action::FocusLeft, Some(WorkspaceIn::Goto(Goto::Spatial(Direction::Left)))),
        (Action::FocusDown, Some(WorkspaceIn::Goto(Goto::Spatial(Direction::Down)))),
        (Action::ToggleZoom, Some(WorkspaceIn::ToggleZoom)),
        (Action::Equalize, Some(WorkspaceIn::Equalize)),
        (Action::ReopenTab, Some(WorkspaceIn::Reopen)),
        (Action::SelectTab(3), Some(nth(3))),
        (Action::SelectTab(0), None),
        (Action::Copy, None),
        (Action::Find, None),
    ];
    for (action, want) in rows {
        assert_eq!(workspace_input(*action), *want, "{action:?}");
    }
}

#[test]
fn the_sidebar_lists_pinned_then_folders_then_the_rest_with_unread_marks_and_miniatures() {
    let hub = hub_after(vec![
        WorkspaceIn::NewTab { cwd: None },
        WorkspaceIn::NewTab { cwd: None },
        WorkspaceIn::NewTab { cwd: None },
        WorkspaceIn::Tabs(TabsIn::Pin {
            tab: TabId(3),
            relaunch: Relaunch("htop".to_owned()),
        }),
        WorkspaceIn::Tabs(TabsIn::NewFolder {
            title: TabTitle("Work".to_owned()),
        }),
        WorkspaceIn::Tabs(TabsIn::MoveToFolder {
            tab: TabId(2),
            folder: Some(FolderId(1)),
        }),
        WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(TabId(1)))),
        WorkspaceIn::Split {
            towards: Direction::Right,
        },
        // News in a tab that is not in front.
        WorkspaceIn::Activity(temor_core::vocab::ids::PaneId(2)),
    ]);
    let data = hub.sidebar();
    let ids = |rows: &[super::TabRow]| rows.iter().map(|r| r.id.0).collect::<Vec<_>>();
    assert_eq!(ids(&data.pinned), [3]);
    assert_eq!(data.folders.len(), 1);
    assert_eq!(data.folders[0].0, "Work");
    assert_eq!(ids(&data.folders[0].1), [2]);
    assert_eq!(ids(&data.loose), [1]);

    let (pinned, folder, loose) = (&data.pinned[0], &data.folders[0].1[0], &data.loose[0]);
    assert_eq!(folder.unread, Unread::Unread);
    assert_eq!(pinned.unread, Unread::Seen);
    assert_eq!(loose.unread, Unread::Seen);
    assert_eq!(loose.tiles.len(), 2, "the split tab has a miniature");
    assert!(pinned.tiles.is_empty() && folder.tiles.is_empty());
    // Two panes side by side fill the miniature between them.
    let widths: f32 = loose.tiles.iter().map(|t| t.width).sum();
    assert!((widths - 100.0).abs() < 0.5, "{:?}", loose.tiles);
}

#[test]
fn the_home_folder_is_written_as_a_tilde() {
    let home = std::path::Path::new("/home/someone");
    #[rustfmt::skip]
    let rows: &[(&str, &str)] = &[
        ("/home/someone", "~"),
        ("/home/someone/src/temor", "~/src/temor"),
        ("/home/someone2/src", "/home/someone2/src"),
        ("/etc", "/etc"),
    ];
    for (path, want) in rows {
        assert_eq!(tidy(path, home), *want, "{path}");
    }
}
