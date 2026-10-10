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

pub(super) fn launch() -> Launch {
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
        // The bar and the quick terminal are the window's, not the workspace's.
        (Action::OpenBar, None),
        (Action::ToggleQuick, None),
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

mod spaces {
    use ds::components::app::spaces::{SpaceId, Spaces};
    use temor_core::space::{Project, Route};
    use temor_core::vocab::text::{Cwd, RoutePattern};
    use temor_core::workspace::WorkspaceIn;

    use super::super::space_route::expand_home;
    use super::{hub_after, launch};
    use crate::hub::Hub;

    fn project(root: Option<&str>, routes: &[&str]) -> Project {
        Project {
            root: root.map(|r| temor_core::vocab::text::ProjectRoot(r.to_owned())),
            routes: routes
                .iter()
                .map(|p| Route {
                    pattern: RoutePattern((*p).to_owned()),
                })
                .collect(),
            ..Project::default()
        }
    }

    #[test]
    fn switching_puts_a_workspace_away_and_brings_it_back_with_its_tabs() {
        let mut hub = hub_after(vec![
            WorkspaceIn::NewTab {
                cwd: Some(Cwd("/a".into())),
            },
            WorkspaceIn::NewTab { cwd: None },
        ]);
        hub.swap_to(SpaceId(1));
        assert_eq!(hub.active_space(), SpaceId(1));
        assert!(
            hub.workspace().tabs.place.tabs.is_empty(),
            "a new Space starts empty"
        );
        hub.swap_to(SpaceId(0));
        assert_eq!(hub.workspace().tabs.place.tabs.len(), 2);
        assert_eq!(
            hub.here().tabs.len(),
            2,
            "the kit remembers the place that was left"
        );
    }

    #[test]
    fn pane_ids_stay_apart_across_spaces() {
        let mut hub = hub_after(vec![
            WorkspaceIn::NewTab { cwd: None },
            WorkspaceIn::NewTab { cwd: None },
        ]);
        assert_eq!(hub.workspace().next_pane, 3);
        hub.book.note(&hub.ws.clone());
        hub.swap_to(SpaceId(1));
        let mut other = std::mem::replace(&mut hub.ws, super::super::spaces::empty_workspace());
        hub.book.lift(&mut other);
        assert_eq!(
            other.next_pane, 3,
            "the next Space's first pane is past the first Space's"
        );
    }

    #[test]
    fn routes_pick_a_space_by_command_or_folder_with_the_home_folder_expanded() {
        let mut hub = Hub::new(launch());
        hub.book.spaces = Spaces::first_run(
            [
                project(None, &[]),
                project(None, &["ssh prod-*", "~/work"]),
                project(None, &["/srv"]),
            ],
            Project::default,
        );
        #[rustfmt::skip]
        let commands: &[(&str, Option<u64>)] = &[
            ("ssh prod-web1", Some(1)),
            ("ssh staging", None),
            ("ls", None),
        ];
        for (command, want) in commands {
            assert_eq!(
                hub.space_for_command(command),
                want.map(SpaceId),
                "{command}"
            );
        }
        #[rustfmt::skip]
        let folders: &[(&str, Option<u64>)] = &[
            ("/home/someone/work", Some(1)),
            ("/home/someone/work/api", Some(1)),
            ("/home/someone/workshop", None),
            ("/srv/www", Some(2)),
            ("/tmp", None),
        ];
        for (folder, want) in folders {
            assert_eq!(hub.space_for_folder(folder), want.map(SpaceId), "{folder}");
        }
    }

    #[test]
    fn a_new_tab_opens_in_the_spaces_project_folder_and_a_named_folder_wins() {
        let mut hub = Hub::new(launch());
        hub.book.spaces = Spaces::first_run([project(Some("/srv/app"), &[])], Project::default);
        let tab = |cwd: Option<&str>| WorkspaceIn::NewTab {
            cwd: cwd.map(|c| Cwd(c.to_owned())),
        };
        assert_eq!(hub.in_project(tab(None)), tab(Some("/srv/app")));
        assert_eq!(hub.in_project(tab(Some("/x"))), tab(Some("/x")));
        assert_eq!(hub.in_project(WorkspaceIn::Reopen), WorkspaceIn::Reopen);
    }

    #[test]
    fn only_a_leading_tilde_is_the_home_folder() {
        let home = std::path::Path::new("/home/someone");
        #[rustfmt::skip]
        let rows: &[(&str, &str)] = &[
            ("~", "/home/someone"),
            ("~/work", "/home/someone/work"),
            ("~other/work", "~other/work"),
            ("ssh ~", "ssh ~"),
            ("/srv", "/srv"),
        ];
        for (pattern, want) in rows {
            let got = expand_home(&RoutePattern((*pattern).to_owned()), home);
            assert_eq!(got.as_str(), *want, "{pattern}");
        }
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
