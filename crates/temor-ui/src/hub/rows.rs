//! What the sidebar shows (PLAN §7): the tab rows in the model's order (pinned, then folders,
//! then the rest), each with the title and folder of its focused pane, its unread mark, and a
//! miniature of its splits. Read from the hub; the view draws it and decides nothing.

use std::sync::Arc;

use ds::prelude::{Emphasis, Selection};
use temor_client::icons::Image;
use temor_client::model::icon::{IconAsk, TabIcon, icon_for, initial_of, programs_of};
use temor_client::model::presence::{AgentState, attention};
use temor_client::model::split::{Closed, SplitTree, WholePx};
use temor_client::model::vocab::ids::{FolderId, PaneId, TabId};
use temor_client::model::workspace::sidebar_plan;
use temor_client::ssh::HostAlias;

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

/// What leads a tab's row (PLAN P22): the running program's app or brand, at the prompt the
/// folder's own icon, else a letter tile.
#[derive(Debug, Clone, PartialEq)]
pub enum RowIcon {
    /// A picture drawn as it is: the icon of the app the program belongs to, or the folder's.
    Image(Image),
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
    /// The folders, depth first: a folder before the folders in it.
    pub folders: Vec<FolderRows>,
    pub loose: Vec<TabRow>,
}

/// One folder of the sidebar and its tabs' rows.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderRows {
    pub id: FolderId,
    pub title: String,
    /// The folder it is listed under; none at the top.
    pub parent: Option<FolderId>,
    pub open: bool,
    pub rows: Vec<TabRow>,
}

impl SidebarData {
    /// Every row, in the sidebar's order.
    pub fn every(&self) -> impl Iterator<Item = &TabRow> {
        self.pinned
            .iter()
            .chain(self.folders.iter().flat_map(|folder| &folder.rows))
            .chain(&self.loose)
    }

    /// The row of tab `id`, wherever it is.
    pub fn row(&self, id: TabId) -> Option<TabRow> {
        self.every().find(|row| row.id == id).cloned()
    }
}

/// What a console's bar knows of its row.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleRow {
    pub tab: TabId,
    pub pinned: bool,
    /// The folder as this computer writes it; none on a host.
    pub place: Option<String>,
    /// Whether the row is a Split View, whose terminals close one by one.
    pub split: bool,
}

/// The side of the miniature's square the layout is computed in.
const MINIATURE: u32 = 1000;

impl Hub {
    /// The sidebar's rows now.
    pub fn sidebar(&self) -> SidebarData {
        let plan = sidebar_plan(&self.book.ws.tabs);
        let take =
            |ids: &[TabId]| -> Vec<TabRow> { ids.iter().filter_map(|id| self.row(*id)).collect() };
        let pinned = take(&plan.pinned);
        let folders = plan
            .folders
            .iter()
            .map(|(folder, ids)| FolderRows {
                id: folder.id,
                title: folder.title.as_str().to_owned(),
                parent: folder.parent,
                open: folder.open,
                rows: take(ids),
            })
            .collect();
        // The plan's loose tabs include those whose folder is gone.
        let loose = take(&plan.loose);
        SidebarData {
            pinned,
            folders,
            loose,
        }
    }

    pub(super) fn row(&self, id: TabId) -> Option<TabRow> {
        let tab = self.book.ws.tabs.tab(id)?;
        let focus = self.book.ws.focus_of(id)?;
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
            .live(pane, |s| s.cwd())
            .flatten()
            .or_else(|| self.book.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()))
            .filter(|_| !on_host);
        let cwd = place
            .as_deref()
            .map(|path| tidy(path, &self.launch.ambient.home));
        let agent = self.agent_state_of_tab(id);
        // News, or an agent that waits or has a result the person has not looked at.
        let news = self.book.ws.tabs.unread.contains(&id) || attention(agent) == Emphasis::Strong;
        let unread = match news {
            true => Unread::Unread,
            false => Unread::Seen,
        };
        let selection = match self.book.ws.front() == Some(id) {
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
        made_as: &temor_client::model::vocab::text::TabTitle,
    ) -> String {
        let read = self.live(pane, |s| {
            (s.title(), s.cwd(), s.running_command(), s.prompts_seen())
        });
        let (title, live_cwd, running, prompts) = read.unwrap_or_default();
        if let Some(host_title) = self.ssh_title(pane) {
            return host_title;
        }
        let cwd = live_cwd.or_else(|| self.book.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()));
        let marking = match prompts {
            0 => Marking::Unmarked,
            _ => Marking::Marked,
        };
        tab_title(NameIn {
            running: running.as_ref().map(|run| run.command.as_str()),
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
            .flatten();
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
            self.icons.icons.app(&program, &path)
        });
        // At the prompt, the icon set on the folder; a host's folders are not on this computer.
        let folder = match (&running, on_host) {
            (None, false) => self.pane_folder(pane),
            _ => None,
        };
        let folder_icon = folder.as_deref().and_then(|dir| {
            let dir = std::path::Path::new(dir);
            let image = self.icons.icons.folder(dir)?;
            Some((dir, image))
        });
        let ask = IconAsk {
            command: running.as_deref(),
            app: found.as_ref().map(|app| app.app.as_path()),
            folder_icon: folder_icon.as_ref().map(|(dir, _)| *dir),
            title,
        };
        match icon_for(&ask, &self.icons.pack.pack) {
            TabIcon::App(_) => found.map_or_else(|| letter(title), |app| RowIcon::Image(app.image)),
            TabIcon::Folder(_) => {
                folder_icon.map_or_else(|| letter(title), |(_, image)| RowIcon::Image(image))
            }
            TabIcon::Brand(brand) => match self.icons.pack.svg(&brand.glyph) {
                Some(svg) => RowIcon::Brand {
                    svg,
                    colour: brand.colour,
                },
                None => letter(title),
            },
            TabIcon::Letter { .. } => letter(title),
        }
    }

    fn tiles(&self, id: TabId, focus: PaneId) -> Vec<Tile> {
        let Some(tab) = self.book.ws.tabs.tab(id) else {
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
        let title = self.live(pane, |s| s.title()).unwrap_or_default();
        if !title.is_empty() {
            return title;
        }
        self.book
            .ws
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
        let title = self.live(pane, |s| s.title()).unwrap_or_default();
        let cwd = self.live(pane, |s| s.cwd()).flatten();
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

    /// The id the next folder made in the Space on screen gets.
    pub fn next_folder(&self) -> FolderId {
        FolderId(self.0.borrow().book.ws.tabs.next_folder)
    }

    /// Whether the Space on screen has `folder`.
    pub fn has_folder(&self, folder: FolderId) -> bool {
        self.0.borrow().book.ws.tabs.folder(folder).is_some()
    }

    /// The row a console's terminal `pane` is in, for its bar's menu: none for the quick
    /// terminal, which is in no row.
    pub fn console_row(&self, pane: PaneId) -> Option<ConsoleRow> {
        let hub = self.0.borrow();
        let tab = hub.book.ws.tab_of(pane)?;
        let sidebar = hub.sidebar();
        let row = sidebar.row(tab)?;
        let terminals = hub
            .book
            .ws
            .tabs
            .tab(tab)?
            .tree
            .panes()
            .into_iter()
            .filter(|pane| !hub.is_viewer(*pane))
            .count();
        Some(ConsoleRow {
            tab,
            pinned: sidebar.pinned.iter().any(|pinned| pinned.id == tab),
            place: row.place,
            split: terminals > 1,
        })
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
