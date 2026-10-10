//! The hub's pure parts: which request a pane action makes, and what the sidebar shows for a
//! workspace. No panes are started; the workspace is built with its step.

use temor_core::split::{Direction, Goto};
use temor_core::vocab::ids::{FolderId, PaneId, TabId};
use temor_core::vocab::text::{Relaunch, TabTitle};
use temor_core::workspace::{NthTab, TabSel, TabsIn, WorkspaceIn, WorkspaceOut, workspace_step};
use temor_pane::keys::Action;
use temor_pane::{OfferId, OfferKind, TransferEnd, TransferEvent};
use temor_transfer::Consent;

use super::keys::workspace_input;
use super::model::Hub;
use super::rows::{Unread, tidy};
use super::shelf::AskKind;
use crate::edge::Ambient;
use crate::view::launch::{Launch, LauncherChoice};
use crate::view::theme::AppearanceSource;

fn launch() -> Launch {
    launch_in(std::path::PathBuf::from("/home/someone"))
}

fn launch_in(home: std::path::PathBuf) -> Launch {
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
fn hub_after(steps: Vec<WorkspaceIn>) -> Hub {
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

/// A hub with one tab and a pane slot for it (no program runs in it), living in `home`.
fn hub_with_pane(home: &std::path::Path) -> Hub {
    let mut hub = Hub::new(launch_in(home.to_path_buf()));
    let (ws, _) = workspace_step(hub.ws.clone(), WorkspaceIn::NewTab { cwd: None });
    hub.ws = ws;
    hub.slots.insert(
        PaneId(1),
        std::rc::Rc::new(std::cell::RefCell::new(crate::view::slot::Slot::default())),
    );
    hub
}

fn offered(n: u64, kind: OfferKind) -> (PaneId, TransferEvent) {
    (
        PaneId(1),
        TransferEvent::Offered {
            offer: OfferId(n),
            kind,
        },
    )
}

#[test]
fn a_program_is_asked_once_and_the_answer_holds_for_its_pane_until_temor_quits() {
    let home = tempfile::tempdir().expect("home");
    let mut hub = hub_with_pane(home.path());
    hub.transfers(vec![offered(1, OfferKind::Files)]);
    let ask = hub.front_ask().expect("the person is asked");
    assert_eq!(ask.kind, AskKind::Files);
    assert!(ask.view().text.contains("Downloads"), "{:?}", ask.view());
    hub.answer_ask(Consent::Allow);
    assert!(hub.front_ask().is_none());
    // The same pane's next offer is let in without a question; an offer a refusal remembered
    // is turned away without one too.
    hub.transfers(vec![offered(2, OfferKind::Files)]);
    assert!(hub.front_ask().is_none());
    hub.shelf
        .grants
        .insert("pane:1".to_owned(), temor_core::remote::Grant::Refused);
    hub.transfers(vec![offered(3, OfferKind::Files)]);
    assert!(hub.front_ask().is_none());
}

#[test]
fn a_request_for_a_file_is_asked_every_time() {
    let home = tempfile::tempdir().expect("home");
    let mut hub = hub_with_pane(home.path());
    for offer in [1, 2] {
        let wanted = TransferEvent::Wanted {
            offer: OfferId(offer),
            name: Some("a.txt".to_owned()),
        };
        hub.transfers(vec![(PaneId(1), wanted)]);
        let ask = hub.front_ask().expect("asked");
        assert!(matches!(ask.kind, AskKind::Request { .. }));
        hub.answer_ask(Consent::Allow);
    }
}

#[test]
fn a_file_lands_in_downloads_under_a_free_name_and_survives_a_restart() {
    let home = tempfile::tempdir().expect("home");
    let downloads = home.path().join("Downloads");
    std::fs::create_dir_all(&downloads).expect("downloads");
    std::fs::write(downloads.join("a.txt"), b"old").expect("an older a.txt");
    let staged = home.path().join("stage").join("offer-1");
    std::fs::create_dir_all(&staged).expect("staging");
    std::fs::write(staged.join("a.txt"), b"new").expect("staged");

    let mut hub = hub_with_pane(home.path());
    hub.transfers(vec![offered(1, OfferKind::Files)]);
    hub.answer_ask(Consent::Allow);
    let file = "a.txt".to_owned();
    hub.transfers(vec![
        (
            PaneId(1),
            TransferEvent::Started {
                offer: OfferId(1),
                file: file.clone(),
                name: "a.txt".to_owned(),
                size: Some(3),
            },
        ),
        (
            PaneId(1),
            TransferEvent::Saved {
                offer: OfferId(1),
                file,
                path: staged.join("a.txt"),
                size: 3,
            },
        ),
        (
            PaneId(1),
            TransferEvent::Ended {
                offer: OfferId(1),
                how: TransferEnd::Finished,
            },
        ),
    ]);
    assert_eq!(
        std::fs::read(downloads.join("a.txt")).expect("kept"),
        b"old"
    );
    assert_eq!(
        std::fs::read(downloads.join("a (2).txt")).expect("placed"),
        b"new"
    );
    assert!(!staged.join("a.txt").exists(), "the staged copy is gone");
    let view = hub.shelf_view();
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].title, "a (2).txt");
    assert_eq!(view.rows[0].state, temor_core::shelf::RowState::Ready);
    assert_eq!(view.unseen, 1);

    // A new run reads the index back: the file is on the shelf, already seen.
    drop(hub);
    let again = hub_with_pane(home.path());
    let view = again.shelf_view();
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].title, "a (2).txt");
    assert_eq!(view.unseen, 0);
}

#[test]
fn a_refused_or_broken_transfer_leaves_no_row_waiting_and_no_question() {
    let home = tempfile::tempdir().expect("home");
    let mut hub = hub_with_pane(home.path());
    hub.transfers(vec![offered(1, OfferKind::Files)]);
    hub.transfers(vec![(
        PaneId(1),
        TransferEvent::Ended {
            offer: OfferId(1),
            how: TransferEnd::Aborted,
        },
    )]);
    assert!(
        hub.front_ask().is_none(),
        "the program left, so nobody waits"
    );

    hub.shelf
        .grants
        .insert("pane:1".to_owned(), temor_core::remote::Grant::Allowed);
    hub.transfers(vec![
        offered(2, OfferKind::Files),
        (
            PaneId(1),
            TransferEvent::Started {
                offer: OfferId(2),
                file: "b.bin".to_owned(),
                name: "b.bin".to_owned(),
                size: Some(10),
            },
        ),
        (
            PaneId(1),
            TransferEvent::Ended {
                offer: OfferId(2),
                how: TransferEnd::Failed,
            },
        ),
    ]);
    let view = hub.shelf_view();
    assert!(matches!(
        view.rows[0].state,
        temor_core::shelf::RowState::Failed { .. }
    ));
}
