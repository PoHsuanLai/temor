//! The session file: round trips, versions, and the lenient rebuild.

use super::restore::{FreshWhy, Restored, decode, encode, launch_plan};
use super::{ClosedState, KeepClosed, Workspace, WorkspaceIn, workspace_step};
use crate::space::Pin;
use crate::split::Direction;
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::{Cwd, Relaunch, ViewerRef};

fn built() -> Workspace {
    let mut ws = Workspace::new(ClosedState::new(KeepClosed::DEFAULT));
    let steps = vec![
        WorkspaceIn::NewTab {
            cwd: Some(Cwd("/home/a".into())),
        },
        WorkspaceIn::Split {
            towards: Direction::Right,
        },
        WorkspaceIn::NewTab {
            cwd: Some(Cwd("/srv".into())),
        },
        WorkspaceIn::Tabs(crate::workspace::TabsIn::Pin {
            tab: TabId(2),
            relaunch: Relaunch("htop".into()),
        }),
    ];
    for step in steps {
        ws = workspace_step(ws, step).0;
    }
    ws
}

fn loaded(text: &str) -> Workspace {
    match decode(text, KeepClosed::DEFAULT) {
        Restored::Loaded(ws) => *ws,
        other => panic!("expected a loaded workspace, got {other:?}"),
    }
}

#[test]
fn a_workspace_survives_a_round_trip() {
    let ws = built();
    let back = loaded(&encode(&ws).unwrap());
    assert_eq!(back.tabs, ws.tabs);
    assert_eq!(back.focus, ws.focus);
    assert_eq!(back.cwds, ws.cwds);
    assert_eq!(back.next_tab, ws.next_tab);
    assert_eq!(back.next_pane, ws.next_pane);
    assert!(back.closed.tabs.is_empty(), "recently closed is not stored");
}

#[test]
fn damaged_or_unversioned_files_start_fresh() {
    #[rustfmt::skip]
    let cases: &[(&str, FreshWhy)] = &[
        ("", FreshWhy::Damaged),
        ("not json", FreshWhy::Damaged),
        ("{}", FreshWhy::Damaged),
        (r#"{"version":"one"}"#, FreshWhy::Damaged),
        (r#"{"version":0}"#, FreshWhy::Damaged),
        (r#"{"version":2}"#, FreshWhy::NewerVersion(2)),
        (r#"{"version":99,"tabs":[]}"#, FreshWhy::NewerVersion(99)),
    ];
    for (text, why) in cases {
        assert_eq!(
            decode(text, KeepClosed::DEFAULT),
            Restored::Fresh(*why),
            "{text}"
        );
    }
}

#[test]
fn a_duplicate_tab_is_dropped_and_stale_references_are_ignored() {
    let text = encode(&built()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    // A second tab that reuses the first tab's pane: it is dropped, the first stays.
    let first = value["place"]["tabs"][0].clone();
    value["place"]["tabs"].as_array_mut().unwrap().push(first);
    // A focus record for a tab that does not exist, and a folder that does not exist.
    value["focus"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"tab": 99, "pane": 1}));
    value["place"]["tabs"][1]["folder"] = serde_json::json!(42);
    let ws = loaded(&value.to_string());
    let ids: Vec<TabId> = ws.tabs.place.tabs.iter().map(|t| t.id).collect();
    assert_eq!(
        ids,
        vec![TabId(1), TabId(2)],
        "the duplicate tab is dropped"
    );
    assert!(!ws.focus.contains_key(&TabId(99)));
    assert_eq!(ws.tabs.tab(TabId(2)).unwrap().folder, None);
}

#[test]
fn the_id_counters_move_past_every_id_in_use() {
    let text = encode(&built()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["next_tab"] = serde_json::json!(0);
    value["next_pane"] = serde_json::json!(1);
    let ws = loaded(&value.to_string());
    assert_eq!(ws.next_tab, 3);
    assert_eq!(ws.next_pane, 4);
}

#[test]
fn a_malformed_tab_is_dropped_alone() {
    let text = encode(&built()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["place"]["tabs"][1] = serde_json::json!({"id": "two"});
    let ws = loaded(&value.to_string());
    let ids: Vec<TabId> = ws.tabs.place.tabs.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![TabId(1)]);
    assert_eq!(ws.focus.keys().copied().collect::<Vec<_>>(), vec![TabId(1)]);
}

#[test]
fn a_place_that_is_not_a_place_or_cannot_count_on_is_damaged() {
    let max = serde_json::json!(u64::MAX);
    let base: serde_json::Value = serde_json::from_str(&encode(&built()).unwrap()).unwrap();
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)] // a table of named rows
    let edits: Vec<(&str, Box<dyn Fn(&mut serde_json::Value)>)> = vec![
        ("place is a number", Box::new(|v: &mut serde_json::Value| v["place"] = serde_json::json!(5))),
        ("place is null", Box::new(|v: &mut serde_json::Value| v["place"] = serde_json::Value::Null)),
        ("place is a list", Box::new(|v: &mut serde_json::Value| v["place"] = serde_json::json!([]))),
        ("tabs is text", Box::new(|v: &mut serde_json::Value| v["place"]["tabs"] = serde_json::json!("x"))),
        ("folders is a map", Box::new(|v: &mut serde_json::Value| v["place"]["folders"] = serde_json::json!({}))),
        ("a tab id at the top", Box::new({ let m = max.clone(); move |v: &mut serde_json::Value| v["place"]["tabs"][1]["id"] = m.clone() })),
        ("a pane id at the top", Box::new({ let m = max.clone(); move |v: &mut serde_json::Value| v["place"]["tabs"][1]["tree"]["nodes"][0]["v"] = m.clone() })),
        ("a folder id at the top", Box::new({ let m = max.clone(); move |v: &mut serde_json::Value| v["place"]["folders"] = serde_json::json!([{"id": m, "title": "f"}]) })),
    ];
    for (name, edit) in edits {
        let mut value = base.clone();
        edit(&mut value);
        assert_eq!(
            decode(&value.to_string(), KeepClosed::DEFAULT),
            Restored::Fresh(FreshWhy::Damaged),
            "{name}"
        );
    }
}

#[test]
fn a_pinned_tab_relaunches_its_command_on_its_first_pane() {
    let ws = built();
    let plan = launch_plan(&ws);
    assert_eq!(plan.len(), 3, "two panes in tab 1, one in tab 2");
    let pinned_first: Vec<_> = plan
        .iter()
        .filter(|s| s.relaunch.is_some())
        .map(|s| (s.tab, s.pane))
        .collect();
    assert_eq!(pinned_first, vec![(TabId(2), PaneId(3))]);
    assert!(matches!(
        ws.tabs.tab(TabId(2)).unwrap().pin,
        Pin::Pinned { .. }
    ));
    let cwd_of_pane_one = plan
        .iter()
        .find(|s| s.pane == PaneId(1))
        .and_then(|s| s.cwd.clone());
    assert_eq!(cwd_of_pane_one, Some(Cwd("/home/a".into())));
}

#[test]
fn a_place_recalled_alone_brings_its_viewer_panes_back_as_viewers() {
    let mut ws = built();
    ws.viewers.insert(PaneId(2), ViewerRef("/notes.txt".into()));
    // A viewer of a pane that is in no tree is not kept.
    ws.viewers.insert(PaneId(9), ViewerRef("/gone.txt".into()));
    let recall = ws.tabs.place.clone().with_viewers(&ws.viewers);
    assert_eq!(recall.viewers.len(), 1);

    let text = serde_json::json!({ "version": 1, "place": recall }).to_string();
    let back = loaded(&text);

    assert_eq!(
        back.viewers.get(&PaneId(2)),
        Some(&ViewerRef("/notes.txt".into()))
    );
    assert_eq!(back.viewers.len(), 1);
    assert!(
        back.tabs.place.viewers.is_empty(),
        "a live place holds none"
    );
    let shells: Vec<PaneId> = launch_plan(&back).iter().map(|s| s.pane).collect();
    assert_eq!(shells, vec![PaneId(3), PaneId(1)], "pinned tab first");
}
