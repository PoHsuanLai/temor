//! Viewer panes in the workspace: they open beside the pane they came from, a tab keeps one,
//! they close and come back with their tab, and the session file keeps them.

use super::restore::{Restored, decode, encode, launch_plan, viewer_plan};
use super::step_tests::{one_tab, run, spawns};
use super::{KeepClosed, RefuseWhy, ViewerOpen, WorkspaceIn, WorkspaceOut};
use crate::split::Direction;
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::ViewerRef;

fn open(file: &str, from: Option<PaneId>) -> WorkspaceIn {
    WorkspaceIn::OpenViewer {
        file: ViewerRef(file.to_owned()),
        from,
    }
}

fn viewers(out: &[WorkspaceOut]) -> Vec<ViewerOpen> {
    out.iter()
        .filter_map(|o| match o {
            WorkspaceOut::Viewer(v) => Some(v.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_file_opens_in_a_split_to_the_right_and_takes_the_keys() {
    let (ws, shell) = one_tab();
    let (ws, out) = run(ws, vec![open("/w/a.txt", Some(shell))]);
    let opened = viewers(&out);
    assert_eq!(opened.len(), 1);
    let pane = opened[0].pane;
    assert_ne!(pane, shell);
    assert_eq!(opened[0].from, Some(shell));
    assert!(spawns(&out).is_empty(), "a viewer starts no program");
    assert_eq!(ws.viewers.get(&pane), Some(&ViewerRef("/w/a.txt".into())));
    assert_eq!(ws.focus_of(TabId(1)), Some(pane));
    assert!(out.contains(&WorkspaceOut::Focused(pane)));
    assert_eq!(out.last(), Some(&WorkspaceOut::Changed));
    // To the right of the shell: going left from the viewer reaches it.
    let (ws, _) = run(
        ws,
        vec![WorkspaceIn::Goto(crate::split::Goto::Spatial(
            Direction::Left,
        ))],
    );
    assert_eq!(ws.focus_of(TabId(1)), Some(shell));
}

#[test]
fn a_second_file_replaces_the_first_instead_of_splitting_again() {
    let (ws, shell) = one_tab();
    let (ws, _) = run(ws, vec![open("/w/a.txt", Some(shell))]);
    let (ws, out) = run(ws, vec![open("/w/b.txt", Some(shell))]);
    let opened = viewers(&out);
    assert_eq!(opened.len(), 1);
    assert_eq!(ws.tabs.tab(TabId(1)).unwrap().tree.panes().len(), 2);
    assert_eq!(ws.viewers.len(), 1);
    assert_eq!(
        ws.viewers.get(&opened[0].pane),
        Some(&ViewerRef("/w/b.txt".into()))
    );
    assert_eq!(ws.focus_of(TabId(1)), Some(opened[0].pane));
}

#[test]
fn walking_to_another_file_is_kept_without_telling_the_pane() {
    let (ws, shell) = one_tab();
    let (ws, out) = run(ws, vec![open("/w/a.txt", Some(shell))]);
    let pane = viewers(&out)[0].pane;
    let file = ViewerRef("/w/b.txt".into());
    let (ws, out) = run(
        ws,
        vec![WorkspaceIn::ViewerShows {
            pane,
            file: file.clone(),
        }],
    );
    assert_eq!(out, vec![WorkspaceOut::Changed]);
    assert_eq!(ws.viewers.get(&pane), Some(&file));
    // A pane that is not a viewer changes nothing.
    let (_, out) = run(ws, vec![WorkspaceIn::ViewerShows { pane: shell, file }]);
    assert!(out.is_empty());
}

#[test]
fn closing_the_viewer_forgets_it_and_keeps_the_shell() {
    let (ws, shell) = one_tab();
    let (ws, out) = run(ws, vec![open("/w/a.txt", Some(shell))]);
    let pane = viewers(&out)[0].pane;
    let (ws, out) = run(ws, vec![WorkspaceIn::ClosePane(Some(pane))]);
    assert!(out.contains(&WorkspaceOut::End(pane)));
    assert!(ws.viewers.is_empty());
    assert_eq!(ws.focus_of(TabId(1)), Some(shell));
}

#[test]
fn a_reopened_tab_shows_its_viewer_and_starts_only_its_shell() {
    let (ws, shell) = one_tab();
    let (ws, out) = run(ws, vec![open("/w/a.txt", Some(shell))]);
    let pane = viewers(&out)[0].pane;
    let (ws, _) = run(ws, vec![WorkspaceIn::CloseTab(TabId(1))]);
    assert_eq!(ws.viewers.len(), 1, "a closed tab keeps its viewer");
    let (ws, out) = run(ws, vec![WorkspaceIn::Reopen]);
    let shells: Vec<PaneId> = spawns(&out).iter().map(|s| s.pane).collect();
    assert_eq!(shells, vec![shell]);
    assert_eq!(
        viewers(&out).iter().map(|v| v.pane).collect::<Vec<_>>(),
        vec![pane]
    );
    assert_eq!(ws.viewers.len(), 1);
}

#[test]
fn refusals_say_why_and_change_nothing() {
    let (ws, shell) = one_tab();
    let cases = [
        ("no file", open("", Some(shell)), RefuseWhy::NoFile),
        (
            "a pane that is gone",
            open("/a", Some(PaneId(99))),
            RefuseWhy::NoSuchPane(PaneId(99)),
        ),
    ];
    for (what, input, why) in cases {
        let (after, out) = run(ws.clone(), vec![input]);
        assert_eq!(out, vec![WorkspaceOut::Refused(why)], "{what}");
        assert_eq!(after, ws, "{what}");
    }
    let (_, out) = run(super::step_tests::fresh(), vec![open("/a", None)]);
    assert_eq!(out, vec![WorkspaceOut::Refused(RefuseWhy::NoFront)]);
}

#[test]
fn the_session_file_keeps_viewers_and_launches_only_shells() {
    let (ws, shell) = one_tab();
    let (ws, out) = run(ws, vec![open("/w/a.txt", Some(shell))]);
    let pane = viewers(&out)[0].pane;
    let Restored::Loaded(back) = decode(&encode(&ws).unwrap(), KeepClosed::DEFAULT) else {
        panic!("the file did not load");
    };
    assert_eq!(back.viewers, ws.viewers);
    let shells: Vec<PaneId> = launch_plan(&back).iter().map(|s| s.pane).collect();
    assert_eq!(shells, vec![shell]);
    let plan = viewer_plan(&back);
    assert_eq!((plan.len(), plan[0].pane), (1, pane));
    // A viewer of a pane the file no longer has is dropped.
    let text = encode(&ws)
        .unwrap()
        .replace(&format!("\"pane\": {}", pane.0), "\"pane\": 404");
    let Restored::Loaded(odd) = decode(&text, KeepClosed::DEFAULT) else {
        panic!("the file did not load");
    };
    assert!(odd.viewers.is_empty());
}

#[test]
fn a_viewer_follows_its_tab_to_another_space() {
    let (ws, shell) = one_tab();
    let (ws, _) = run(ws, vec![open("/w/a.txt", Some(shell))]);
    let there = super::step_tests::fresh();
    let (here, there) = crate::space::carry_tab(&ws, TabId(1), &there).unwrap();
    assert!(here.viewers.is_empty());
    assert_eq!(there.viewers.len(), 1);
}
