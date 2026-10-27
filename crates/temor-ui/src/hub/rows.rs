//! What the sidebar shows (PLAN §7): the tab rows in the model's order (pinned, then folders,
//! then the rest), each with the title and folder of its focused pane, its unread mark, and a
//! miniature of its splits. Read from the hub; the view draws it and decides nothing.

use std::sync::Arc;

use ds::prelude::{Emphasis, Selection};
use temor_core::icon::{IconAsk, TabIcon, icon_for, initial_of, programs_of};
use temor_core::presence::{AgentState, attention};
use temor_core::split::{Closed, SplitTree, WholePx};
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::sidebar_plan;
use temor_ssh::HostAlias;

use super::model::{Hub, HubRef};
use super::tab_name::{Marking, NameIn, tab_title};

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
    /// The agent in this pane, if one works or waits there: its segment shows the orb.
    pub agent: AgentState,
}

/// What leads a tab's row (PLAN P22): the running program's app or brand, else a letter tile.
#[derive(Debug, Clone, PartialEq)]
pub enum RowIcon {
    /// The icon of the Mac app the program belongs to, as a PNG.
    App(Arc<[u8]>),
    /// A brand's glyph, in white on its colour (`0xRRGGBB`).
    Brand { svg: Arc<str>, colour: u32 },
    /// A letter on a tile coloured from `key`.
    Letter { initial: char, key: String },
}

/// One tab's row.
#[derive(Debug, Clone, PartialEq)]
pub struct TabRow {
    pub id: TabId,
    pub title: String,
    pub cwd: Option<String>,
    /// The folder as this computer writes it (an absolute path); none on a host, or not known.
    pub place: Option<String>,
    pub unread: Unread,
    /// What the agent in the tab is doing: the most urgent of its panes.
    pub agent: AgentState,
    /// What plugins say about the pane's folder, on one line, for hover.
    pub about: Option<String>,
    /// Selected when the tab is in front.
    pub selection: Selection,
    /// The pane with the keys in this tab.
    pub pane: PaneId,
    pub icon: RowIcon,
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

impl SidebarData {
    /// The row of tab `id`, wherever it is.
    pub fn row(&self, id: TabId) -> Option<TabRow> {
        self.pinned
            .iter()
            .chain(&self.loose)
            .chain(self.folders.iter().flat_map(|(_, rows)| rows))
            .find(|row| row.id == id)
            .cloned()
    }
}

/// The side of the miniature's square the layout is computed in.
const MINIATURE: u32 = 1000;

impl Hub {
    /// The sidebar's rows now.
    pub fn sidebar(&self) -> SidebarData {
        let plan = sidebar_plan(&self.ws.tabs);
        let take =
            |ids: &[TabId]| -> Vec<TabRow> { ids.iter().filter_map(|id| self.row(*id)).collect() };
        let pinned = take(&plan.pinned);
        let folders = plan
            .folders
            .iter()
            .map(|(folder, ids)| (folder.title.as_str().to_owned(), take(ids)))
            .collect();
        // The plan's loose tabs include those whose folder is gone.
        let loose = take(&plan.loose);
        SidebarData {
            pinned,
            folders,
            loose,
        }
    }

    fn row(&self, id: TabId) -> Option<TabRow> {
        let tab = self.ws.tabs.tab(id)?;
        let focus = self.ws.focus_of(id)?;
        // A page with the keys names the row, as a browser tab is named by its page; the folder
        // is still its terminal's.
        let page_name = self.viewer_name(focus);
        let pane = match page_name {
            Some(_) => self
                .console_of(&tab.tree)
                .panes()
                .into_iter()
                .find(|pane| !self.is_viewer(*pane))
                .unwrap_or(focus),
            None => focus,
        };
        let title = page_name.unwrap_or_else(|| self.pane_title(pane, &tab.title));
        // A host's title already names its folder, and its paths are not under this home.
        let on_host = self.host_of(pane).is_some();
        let place = self
            .live(pane, |s| s.cwd().map(str::to_owned))
            .flatten()
            .or_else(|| self.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()))
            .filter(|_| !on_host);
        let cwd = place
            .as_deref()
            .map(|path| tidy(path, &self.launch.ambient.home));
        let agent = self.agent_state_of_tab(id);
        // News, or an agent that waits or has a result the person has not looked at.
        let news = self.ws.tabs.unread.contains(&id) || attention(agent) == Emphasis::Strong;
        let unread = match news {
            true => Unread::Unread,
            false => Unread::Seen,
        };
        let selection = match self.ws.front() == Some(id) {
            true => Selection::Selected,
            false => Selection::Unselected,
        };
        // A page is the row's stage, not a pane beside it: only the terminals split the row.
        let tiles = match self.console_of(&tab.tree).panes().len() > 1 {
            true => self.tiles(id, pane),
            false => Vec::new(),
        };
        let icon = match self.is_viewer(focus) {
            true => letter(&title),
            false => self.row_icon(pane, &title, on_host),
        };
        Some(TabRow {
            id,
            icon,
            title,
            cwd,
            place,
            unread,
            agent,
            about: self.plugin_facts_summary(pane),
            selection,
            pane,
            tiles,
        })
    }

    /// The name a tab whose focus is `pane` goes by (see `tab_name`).
    pub(super) fn pane_title(
        &self,
        pane: PaneId,
        made_as: &temor_core::vocab::text::TabTitle,
    ) -> String {
        let read = self.live(pane, |s| {
            (
                s.title().to_owned(),
                s.cwd().map(str::to_owned),
                s.running_command(),
                s.prompts_seen(),
            )
        });
        let (title, live_cwd, running, prompts) = read.unwrap_or_default();
        if let Some(host_title) = self.ssh_title(pane) {
            return host_title;
        }
        let cwd = live_cwd.or_else(|| self.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()));
        let marking = match prompts {
            0 => Marking::Unmarked,
            _ => Marking::Marked,
        };
        // A live card's pane runs the program it was given, which its own shell never marked.
        let carried = self.live_head(pane).map(|head| head.command);
        tab_title(NameIn {
            running: running
                .as_ref()
                .map(|run| run.command.as_str())
                .or(carried.as_deref()),
            title: &title,
            cwd: cwd.as_deref(),
            home: &self.launch.ambient.home,
            fallback: made_as.as_str(),
            marking,
        })
    }

    /// The icon of the row whose terminal is `pane`, called `title`: what runs in it, by the app
    /// it belongs to (on this computer) or its brand; the letter tile at the prompt or for a
    /// program with neither.
    fn row_icon(&self, pane: PaneId, title: &str, on_host: bool) -> RowIcon {
        let running = self
            .live(pane, |s| s.running_command().map(|run| run.command))
            .flatten()
            .or_else(|| self.live_head(pane).map(|head| head.command));
        let found = running.as_deref().and_then(|command| {
            let program = programs_of(command).into_iter().next()?;
            // Found where the shell finds it; a host's programs are not on this computer.
            let path = match on_host {
                true => return None,
                false => self.live(pane, |s| s.shell_path().map(std::ffi::OsString::from)),
            };
            let path = path
                .flatten()
                .or_else(|| self.launch.ambient.env.get("PATH").map(ToOwned::to_owned))?;
            self.app_icons.borrow_mut().of(&program, &path)
        });
        let ask = IconAsk {
            command: running.as_deref(),
            app: found.as_ref().map(|app| app.app.as_path()),
            folder_icon: None,
            title,
        };
        match icon_for(&ask, &self.icon_pack.pack) {
            TabIcon::App(_) => found.map_or_else(|| letter(title), |app| RowIcon::App(app.png)),
            TabIcon::Brand(brand) => match self.icon_pack.svg(&brand.glyph) {
                Some(svg) => RowIcon::Brand {
                    svg,
                    colour: brand.colour,
                },
                None => letter(title),
            },
            TabIcon::Folder(_) | TabIcon::Letter { .. } => letter(title),
        }
    }

    fn tiles(&self, id: TabId, focus: PaneId) -> Vec<Tile> {
        let Some(tab) = self.ws.tabs.tab(id) else {
            return Vec::new();
        };
        let whole = MINIATURE as f32;
        let tree = self.console_of(&tab.tree);
        let rects = tree.layout(WholePx(MINIATURE), WholePx(MINIATURE));
        // The panes as they stand, which is the order the focus steps through them (⌘]).
        tree.reading_order()
            .into_iter()
            .filter_map(|pane| {
                let (_, rect) = rects.iter().find(|(at, _)| *at == pane)?;
                let percent = |v: WholePx| v.0 as f32 * 100.0 / whole;
                Some(Tile {
                    pane,
                    left: percent(rect.x),
                    top: percent(rect.y),
                    width: percent(rect.width),
                    height: percent(rect.height),
                    // Named as a tab is: its folder, or what runs in it.
                    title: match self.viewer_name(pane) {
                        Some(name) => name,
                        None => self.pane_title(pane, &tab.title),
                    },
                    selection: match pane == focus {
                        true => Selection::Selected,
                        false => Selection::Unselected,
                    },
                    agent: self.agents.radar.most_urgent([pane]),
                })
            })
            .collect()
    }

    /// The row's console: its panes but the page on its stage.
    fn console_of(&self, tree: &SplitTree) -> SplitTree {
        let page = tree.panes().into_iter().find(|pane| self.is_viewer(*pane));
        match page.map(|page| tree.remove(page)) {
            Some(Ok(Closed::Remaining(rest))) => rest,
            _ => tree.clone(),
        }
    }

    /// What a pane is called: its program's title, else the last part of its folder, else "Shell".
    pub(super) fn pane_name(&self, pane: PaneId) -> String {
        if let Some(name) = self.viewer_name(pane) {
            return name;
        }
        if let Some(title) = self.ssh_title(pane) {
            return title;
        }
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

    /// The host `pane` is a session on, if it is one.
    pub(super) fn alias_of(&self, pane: PaneId) -> Option<HostAlias> {
        self.ssh.panes.get(&pane).map(|host| host.alias.clone())
    }

    /// Changes the pane's session, if it has one. The mutable twin of [`Hub::live`].
    pub(super) fn live_mut<T>(
        &self,
        pane: PaneId,
        change: impl FnOnce(&mut crate::session::Session) -> T,
    ) -> Option<T> {
        let slot = self.slots.get(&pane)?;
        let mut slot = slot.borrow_mut();
        slot.session.as_mut().map(change)
    }

    /// Reads something from the pane's session, if it has one.
    pub(super) fn live<T>(
        &self,
        pane: PaneId,
        read: impl FnOnce(&crate::session::Session) -> T,
    ) -> Option<T> {
        let slot = self.slots.get(&pane)?;
        let slot = slot.borrow();
        slot.session.as_ref().map(read)
    }

    /// The title and folder the window shows for the pane that has the keyboard.
    pub fn window_parts(&self) -> (String, Option<String>) {
        let Some(pane) = self.keyboard_pane() else {
            return (String::new(), None);
        };
        if let Some(title) = self.ssh_title(pane) {
            return (title, None);
        }
        if let Some(name) = self.viewer_name(pane) {
            return (name, None);
        }
        let title = self
            .live(pane, |s| s.title().to_owned())
            .unwrap_or_default();
        let cwd = self.live(pane, |s| s.cwd().map(str::to_owned)).flatten();
        (title, cwd)
    }
}

/// The letter tile for `title`.
fn letter(title: &str) -> RowIcon {
    RowIcon::Letter {
        initial: initial_of(title),
        key: title.to_owned(),
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

    /// The person's home folder, as a path.
    pub fn home(&self) -> String {
        self.0
            .borrow()
            .launch
            .ambient
            .home
            .to_string_lossy()
            .into_owned()
    }

    /// The window's title parts: the keyboard pane's title and folder.
    pub fn window_parts(&self) -> (String, Option<String>) {
        self.0.borrow().window_parts()
    }
}
