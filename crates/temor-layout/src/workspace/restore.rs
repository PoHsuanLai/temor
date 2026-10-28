//! The session file's contents (format v1, PLAN §7 "session is restored on launch"): the tab
//! list with its trees, each tab's focused pane, each pane's folder, and the id counters. The
//! recently closed list and scrollback are not stored here.
//!
//! Versioning: the file holds a top-level `version`. A file this build cannot read is never
//! overwritten: a newer version is refused as [`FreshWhy::NewerVersion`]; a missing or damaged
//! file as [`FreshWhy::Damaged`]. Reading is lenient like [`Place`]'s: a damaged tab is dropped
//! alone, and a reference to a missing tab, folder or pane is ignored.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::closed::{ClosedState, KeepClosed};
use super::model::Workspace;
use super::sidebar::order;
use super::step::relaunch_for;
use super::step_types::{Spawn, Start, ViewerOpen};
use super::tabs::TabsState;
use crate::space::Place;
use temor_core::error::CoreError;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::{Cwd, ViewerRef};
use temor_schema::lenient::{lenient_list, lenient_or_default};

/// The format version this build writes and reads.
pub const VERSION: u32 = 1;

/// Why a session file was not restored: the window starts fresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FreshWhy {
    /// The file is missing its version, is not JSON, or is an unknown old version.
    Damaged,
    /// The file was written by a newer build; it is left alone.
    NewerVersion(u32),
}

/// What reading a session file gave.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Restored {
    /// The saved workspace.
    Loaded(Box<Workspace>),
    /// Nothing usable: start fresh.
    Fresh(FreshWhy),
}

#[derive(Serialize)]
struct WriteV1<'a> {
    version: u32,
    place: &'a Place,
    focus: Vec<FocusRec>,
    cwds: Vec<CwdRec>,
    viewers: Vec<ViewerRec>,
    next_tab: u64,
    next_pane: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
struct ViewerRec {
    pane: PaneId,
    file: ViewerRef,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
struct FocusRec {
    tab: TabId,
    pane: PaneId,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
struct CwdRec {
    pane: PaneId,
    cwd: Cwd,
}

#[derive(Deserialize)]
struct ReadV1 {
    #[serde(default, deserialize_with = "lenient_or_default")]
    place: Place,
    #[serde(default, deserialize_with = "lenient_list")]
    focus: Vec<FocusRec>,
    #[serde(default, deserialize_with = "lenient_list")]
    cwds: Vec<CwdRec>,
    #[serde(default, deserialize_with = "lenient_list")]
    viewers: Vec<ViewerRec>,
    #[serde(default)]
    next_tab: u64,
    #[serde(default)]
    next_pane: u64,
}

/// The session file's text for `ws`. Only what a restart needs is written.
pub fn encode(ws: &Workspace) -> Result<String, CoreError> {
    let panes: BTreeSet<PaneId> = ws.tabs.panes().into_iter().collect();
    let write = WriteV1 {
        version: VERSION,
        place: &ws.tabs.place,
        focus: ws
            .focus
            .iter()
            .map(|(tab, pane)| FocusRec {
                tab: *tab,
                pane: *pane,
            })
            .collect(),
        cwds: ws
            .cwds
            .iter()
            .filter(|(pane, _)| panes.contains(pane))
            .map(|(pane, cwd)| CwdRec {
                pane: *pane,
                cwd: cwd.clone(),
            })
            .collect(),
        viewers: ws
            .viewers
            .iter()
            .filter(|(pane, _)| panes.contains(pane))
            .map(|(pane, file)| ViewerRec {
                pane: *pane,
                file: file.clone(),
            })
            .collect(),
        next_tab: ws.next_tab,
        next_pane: ws.next_pane,
    };
    serde_json::to_string_pretty(&write).map_err(|e| CoreError::SessionEncode(e.to_string()))
}

/// The workspace in `text`, or why it starts fresh. `keep` sets how many closed tabs there
/// will be (they are not stored, so the list starts empty).
pub fn decode(text: &str, keep: KeepClosed) -> Restored {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Restored::Fresh(FreshWhy::Damaged);
    };
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    match version {
        Some(VERSION) => {}
        Some(newer) if newer > VERSION => return Restored::Fresh(FreshWhy::NewerVersion(newer)),
        _ => return Restored::Fresh(FreshWhy::Damaged),
    }
    if !place_is_well_formed(&value) {
        return Restored::Fresh(FreshWhy::Damaged);
    }
    match serde_json::from_value::<ReadV1>(value) {
        Ok(read) => rebuild(read, keep).map_or(Restored::Fresh(FreshWhy::Damaged), |ws| {
            Restored::Loaded(Box::new(ws))
        }),
        Err(_) => Restored::Fresh(FreshWhy::Damaged),
    }
}

/// Whether `place` is shaped like a place: absent (an empty one), or a map whose `tabs` and
/// `folders`, when present, are lists. A single damaged tab inside is dropped alone later; a
/// place that is not a place at all is damage the edge must move the file aside for, not an
/// empty workspace to save over it.
fn place_is_well_formed(file: &serde_json::Value) -> bool {
    let Some(place) = file.get("place") else {
        return true;
    };
    let Some(map) = place.as_object() else {
        return false;
    };
    ["tabs", "folders"]
        .iter()
        .all(|key| map.get(*key).is_none_or(serde_json::Value::is_array))
}

/// The saved parts made consistent: duplicate tabs and panes are dropped (the first one stays),
/// references to missing things are ignored, and the id counters move past every id in use.
/// An id at the top of the range leaves none after it: that file is damaged.
fn rebuild(read: ReadV1, keep: KeepClosed) -> Option<Workspace> {
    let mut place = read.place;
    // Viewers a Spaces-file place carries: used only for panes the session list does not name.
    let recalled = std::mem::take(&mut place.viewers);
    let recalled_cwds = std::mem::take(&mut place.cwds);
    let mut seen_tabs = BTreeSet::new();
    let mut seen_panes = BTreeSet::new();
    place.tabs.retain(|tab| {
        let panes = tab.tree.panes();
        let fresh = seen_tabs.insert(tab.id) && panes.iter().all(|p| !seen_panes.contains(p));
        if fresh {
            seen_panes.extend(panes);
        }
        fresh
    });
    let mut folders = BTreeSet::new();
    place.folders.retain(|f| folders.insert(f.id));
    for tab in &mut place.tabs {
        if tab.folder.is_some_and(|f| !folders.contains(&f)) {
            tab.folder = None;
        }
    }
    let ids: BTreeSet<TabId> = place.tabs.iter().map(|t| t.id).collect();
    if place.focused.is_some_and(|f| !ids.contains(&f)) {
        place.focused = None;
    }
    let mut ws = Workspace::new(ClosedState::new(keep));
    let mut tabs = TabsState::from_place(place);
    if tabs.place.focused.is_none() {
        tabs.place.focused = order(&tabs).first().copied();
    }
    ws.focus = read
        .focus
        .iter()
        .filter(|rec| tabs.tab(rec.tab).is_some_and(|t| t.tree.contains(rec.pane)))
        .map(|rec| (rec.tab, rec.pane))
        .collect();
    // The folders a Spaces-file place carries: the session's own record of a pane wins.
    ws.cwds = recalled_cwds
        .into_iter()
        .map(|rec| CwdRec {
            pane: rec.pane,
            cwd: rec.cwd,
        })
        .chain(read.cwds)
        .filter(|rec| seen_panes.contains(&rec.pane))
        .map(|rec| (rec.pane, rec.cwd))
        .collect::<BTreeMap<_, _>>();
    ws.viewers = recalled
        .into_iter()
        .map(|rec| ViewerRec {
            pane: rec.pane,
            file: rec.file,
        })
        .chain(read.viewers)
        .filter(|rec| seen_panes.contains(&rec.pane) && !rec.file.as_str().is_empty())
        .map(|rec| (rec.pane, rec.file))
        .collect();
    if tabs.place.folders.iter().any(|f| f.id.0 == u64::MAX) {
        return None;
    }
    let top_tab = ids
        .iter()
        .map(|t| t.0.checked_add(1))
        .try_fold(1, |top: u64, next| next.map(|n| top.max(n)))?;
    let top_pane = seen_panes
        .iter()
        .map(|p| p.0.checked_add(1))
        .try_fold(1, |top: u64, next| next.map(|n| top.max(n)))?;
    ws.next_tab = read.next_tab.max(top_tab).max(1);
    ws.next_pane = read.next_pane.max(top_pane).max(1);
    ws.tabs = tabs;
    Some(ws)
}

/// The shells to start when a saved workspace is restored: every pane in every tab but the
/// viewers, each in its folder, and a pinned tab's first pane running its command again.
pub fn launch_plan(ws: &Workspace) -> Vec<Spawn> {
    order(&ws.tabs)
        .into_iter()
        .filter_map(|id| ws.tabs.tab(id).map(|tab| (id, tab)))
        .flat_map(|(id, tab)| {
            tab.tree
                .panes()
                .into_iter()
                .filter(|pane| !ws.viewers.contains_key(pane))
                .map(move |pane| Spawn {
                    pane,
                    tab: id,
                    cwd: ws.cwds.get(&pane).cloned(),
                    relaunch: relaunch_for(tab, pane),
                    start: Start::Fresh,
                })
        })
        .collect()
}

/// The viewer panes to show when a saved workspace is restored, in tab order. The caller checks
/// that each file is still there and closes the pane of one that is not.
pub fn viewer_plan(ws: &Workspace) -> Vec<ViewerOpen> {
    order(&ws.tabs)
        .into_iter()
        .filter_map(|id| ws.tabs.tab(id).map(|tab| (id, tab)))
        .flat_map(|(id, tab)| {
            tab.tree.panes().into_iter().filter_map(move |pane| {
                ws.viewers.get(&pane).map(|file| ViewerOpen {
                    pane,
                    tab: id,
                    file: file.clone(),
                    from: None,
                })
            })
        })
        .collect()
}
