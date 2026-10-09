//! What the sidebar shows (PLAN §7): the tab rows in the model's order (pinned, then folders,
//! then the rest), each with the title and folder of its focused pane, its unread mark, and a
//! miniature of its splits. Read from the hub; the view draws it and decides nothing.

use std::collections::BTreeSet;

use ds::prelude::Selection;
use temor_core::split::WholePx;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::sidebar_plan;

use super::model::{Hub, HubRef};

/// What a drag in the sidebar carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DragKey {
    /// A whole tab, by its row.
    Tab(TabId),
    /// One pane, by its tile in a row's miniature.
    Tile(PaneId),
}

/// Whether a tab has news the person has not seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unread {
    Seen,
    Unread,
}

/// One pane of a tab drawn small, placed in percent of the miniature.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub pane: PaneId,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub title: String,
    /// Selected when this pane has the keys in its tab.
    pub selection: Selection,
}

/// One tab's row.
#[derive(Debug, Clone, PartialEq)]
pub struct TabRow {
    pub id: TabId,
    pub title: String,
    pub cwd: Option<String>,
    pub unread: Unread,
    /// Selected when the tab is in front.
    pub selection: Selection,
    /// The pane with the keys in this tab.
    pub pane: PaneId,
    /// The panes drawn small; empty for a tab with a single pane.
    pub tiles: Vec<Tile>,
}

/// The sidebar's sections.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SidebarData {
    pub pinned: Vec<TabRow>,
    pub folders: Vec<(String, Vec<TabRow>)>,
    pub loose: Vec<TabRow>,
}

/// The side of the miniature's square the layout is computed in.
const MINIATURE: u32 = 1000;

impl Hub {
    /// The sidebar's rows now.
    pub fn sidebar(&self) -> SidebarData {
        let plan = sidebar_plan(&self.ws.tabs);
        let mut listed = BTreeSet::new();
        let mut take = |ids: &[TabId]| -> Vec<TabRow> {
            ids.iter()
                .filter_map(|id| self.row(*id))
                .inspect(|row| {
                    listed.insert(row.id);
                })
                .collect()
        };
        let pinned = take(&plan.pinned);
        let folders = plan
            .folders
            .iter()
            .map(|(folder, ids)| (folder.title.as_str().to_owned(), take(ids)))
            .collect();
        let mut loose = take(&plan.loose);
        // A tab in a folder that is gone is still a tab: it is listed with the rest.
        for tab in &self.ws.tabs.place.tabs {
            if !listed.contains(&tab.id) && !loose.iter().any(|row| row.id == tab.id) {
                loose.extend(self.row(tab.id));
            }
        }
        SidebarData {
            pinned,
            folders,
            loose,
        }
    }

    fn row(&self, id: TabId) -> Option<TabRow> {
        let tab = self.ws.tabs.tab(id)?;
        let pane = self.ws.focus_of(id)?;
        let title = self
            .live(pane, |s| s.title().to_owned())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| tab.title.as_str().to_owned());
        let cwd = self
            .live(pane, |s| s.cwd().map(str::to_owned))
            .flatten()
            .or_else(|| self.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()))
            .map(|path| tidy(&path, &self.launch.ambient.home));
        let unread = match self.ws.tabs.unread.contains(&id) {
            true => Unread::Unread,
            false => Unread::Seen,
        };
        let selection = match self.ws.front() == Some(id) {
            true => Selection::Selected,
            false => Selection::Unselected,
        };
        let tiles = match tab.tree.panes().len() > 1 {
            true => self.tiles(id, pane),
            false => Vec::new(),
        };
        Some(TabRow {
            id,
            title,
            cwd,
            unread,
            selection,
            pane,
            tiles,
        })
    }

    fn tiles(&self, id: TabId, focus: PaneId) -> Vec<Tile> {
        let Some(tab) = self.ws.tabs.tab(id) else {
            return Vec::new();
        };
        let whole = MINIATURE as f32;
        tab.tree
            .layout(WholePx(MINIATURE), WholePx(MINIATURE))
            .into_iter()
            .map(|(pane, rect)| {
                let percent = |v: WholePx| v.0 as f32 * 100.0 / whole;
                Tile {
                    pane,
                    left: percent(rect.x),
                    top: percent(rect.y),
                    width: percent(rect.width),
                    height: percent(rect.height),
                    title: self.pane_name(pane),
                    selection: match pane == focus {
                        true => Selection::Selected,
                        false => Selection::Unselected,
                    },
                }
            })
            .collect()
    }

    /// What a pane is called: its program's title, else the last part of its folder, else "Shell".
    fn pane_name(&self, pane: PaneId) -> String {
        let title = self
            .live(pane, |s| s.title().to_owned())
            .unwrap_or_default();
        if !title.is_empty() {
            return title;
        }
        self.ws
            .cwds
            .get(&pane)
            .and_then(|c| c.as_str().rsplit('/').find(|part| !part.is_empty()))
            .map_or_else(|| "Shell".to_owned(), str::to_owned)
    }

    /// Reads something from the pane's session, if it has one.
    fn live<T>(&self, pane: PaneId, read: impl FnOnce(&crate::session::Session) -> T) -> Option<T> {
        let slot = self.slots.get(&pane)?;
        let slot = slot.borrow();
        slot.session.as_ref().map(read)
    }

    /// The title and folder the window shows for the pane that has the keyboard.
    pub fn window_parts(&self) -> (String, Option<String>) {
        let Some(pane) = self.keyboard_pane() else {
            return (String::new(), None);
        };
        let title = self
            .live(pane, |s| s.title().to_owned())
            .unwrap_or_default();
        let cwd = self.live(pane, |s| s.cwd().map(str::to_owned)).flatten();
        (title, cwd)
    }
}

/// A folder as the sidebar shows it: the home folder written as `~`.
pub(super) fn tidy(path: &str, home: &std::path::Path) -> String {
    let home = home.to_string_lossy();
    match path.strip_prefix(home.as_ref()) {
        Some("") => "~".to_owned(),
        Some(rest) if rest.starts_with('/') && !home.is_empty() => format!("~{rest}"),
        _ => path.to_owned(),
    }
}

impl HubRef {
    /// The sidebar's rows now.
    pub fn sidebar(&self) -> SidebarData {
        self.0.borrow().sidebar()
    }

    /// The window's title parts: the keyboard pane's title and folder.
    pub fn window_parts(&self) -> (String, Option<String>) {
        self.0.borrow().window_parts()
    }
}
