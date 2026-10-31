//! Each row's stage and console (PLAN §7, v14): the page a command opened beside its terminal,
//! what the row shows of the two, and Back and Forward through the pages it has shown. The page
//! is the tab's viewer pane; the rules for walking the pages are core's (`stage::Pages`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use temor_client::model::address::{Crumb, crumbs_of};
use temor_client::model::stage::{Layout, Pages};
use temor_client::model::vocab::ids::{PaneId, TabId};
use temor_client::model::vocab::text::ViewerRef;
use temor_client::model::workspace::WorkspaceIn;
use temor_schema::url::{HistoryScope, Host, Location, TemorUrl};

use super::desk::Desk;
use super::model::{Hub, HubRef};
use crate::view::pop_out::PoppedOut;

/// What the hub keeps of every row's stage.
#[derive(Default)]
pub(super) struct Stages {
    rows: BTreeMap<TabId, Stage>,
    /// Set while Back or Forward opens a page, so opening it is not a step of its own.
    walking: bool,
}

#[derive(Default)]
struct Stage {
    layout: Layout,
    pages: Pages<PathBuf>,
    /// The viewer pane the page is in, while there is one.
    viewer: Option<PaneId>,
}

/// What the window draws of a row's stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageView {
    pub layout: Layout,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

/// What the address at the top of the sidebar says of the row in front.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Address {
    /// Nothing: no row, or a terminal whose folder is not known or is on a host.
    #[default]
    None,
    /// One of temor's own pages that is no folder.
    Page(TemorUrl),
    /// The folders down to what is shown; `file` when the last is a file, not a folder.
    Path { crumbs: Vec<Crumb>, file: bool },
}

/// The address of `path` under `home`.
fn path_address(path: &str, home: &str, file: bool) -> Address {
    match crumbs_of(path, home) {
        crumbs if crumbs.is_empty() => Address::None,
        crumbs => Address::Path { crumbs, file },
    }
}

/// The page of temor's own `file` names (`temor://history`, a folder on this computer), when it
/// names one and not a file.
pub fn temor_page(file: &str) -> Option<TemorUrl> {
    match file.parse::<TemorUrl>().ok()? {
        TemorUrl::Location(place) => folder_page(&place).then_some(TemorUrl::Location(place)),
        page => Some(page),
    }
}

/// Whether `place` is a folder's page: a folder on this computer, not a session in it.
fn folder_page(place: &Location) -> bool {
    place.host == Host::Local && place.is_folder() && place.within.is_none()
}

/// The page of the folder `path` on this computer.
pub fn folder_url(path: &str) -> TemorUrl {
    let path = match path.ends_with('/') {
        true => path.to_owned(),
        false => format!("{path}/"),
    };
    TemorUrl::Location(Location {
        host: Host::Local,
        path,
        within: None,
    })
}

/// Where on this computer the page `file` is: a folder's page is its folder, any other page its
/// file.
pub fn page_path(file: &str) -> PathBuf {
    match temor_page(file) {
        Some(TemorUrl::Location(place)) => match place.path.trim_end_matches('/') {
            "" => PathBuf::from("/"),
            folder => PathBuf::from(folder),
        },
        _ => PathBuf::from(file),
    }
}

/// What a page of temor's own is called: a folder by its name.
pub fn temor_page_name(page: &TemorUrl) -> String {
    match page {
        TemorUrl::Downloads => "Downloads".to_owned(),
        TemorUrl::History(_) => "History".to_owned(),
        TemorUrl::Location(place) => Path::new(&place.path).file_name().map_or_else(
            || place.path.clone(),
            |name| name.to_string_lossy().into_owned(),
        ),
    }
}

impl Hub {
    /// Open temor's page `url` on the stage of the row in front, beside the terminal with the
    /// keys (or the one the page there came from).
    pub(super) fn open_page(&mut self, url: TemorUrl) {
        self.open_on_stage(ViewerRef(url.to_string()));
    }

    /// Open `file` (a file's path, or temor's page) on the stage of the row in front.
    pub(super) fn open_on_stage(&mut self, file: ViewerRef) {
        let from = self
            .keyboard_pane()
            .and_then(|pane| match self.is_viewer(pane) {
                true => self.viewer_unfocus_to(pane),
                false => Some(pane),
            });
        self.act(WorkspaceIn::OpenViewer { file, from });
    }

    /// A page opened in `viewer`, on the stage of `tab`: a step Back can return from, and shown.
    pub(super) fn stage_opened(&mut self, tab: TabId, viewer: PaneId, file: PathBuf) {
        let walking = self.stages.walking;
        let own_page = temor_page(&file.to_string_lossy()).is_some();
        let stage = self.stages.rows.entry(tab).or_default();
        stage.viewer = Some(viewer);
        if !walking {
            stage.pages.visit(file);
        }
        // A page asked for is a page shown; temor's own pages are shown alone, as a browser's
        // history is.
        if own_page {
            stage.layout = Layout::Stage;
        } else if !stage.layout.shows_stage() {
            stage.layout = Layout::Both;
        }
    }

    /// The viewer pane `viewer` ended: its row's stage is empty, and Back opens its page again
    /// (unless Back itself closed it, for the console).
    pub(super) fn stage_closed(&mut self, viewer: PaneId) {
        let walking = self.stages.walking;
        if let Some(stage) = self
            .stages
            .rows
            .values_mut()
            .find(|stage| stage.viewer == Some(viewer))
        {
            stage.viewer = None;
            if !walking {
                stage.pages.close();
            }
            stage.layout = Layout::default();
        }
    }

    /// What the window draws of `tab`'s stage.
    pub fn stage_view(&self, tab: TabId) -> StageView {
        let Some(stage) = self.stages.rows.get(&tab) else {
            return StageView {
                layout: Layout::Console,
                ..StageView::default()
            };
        };
        StageView {
            layout: match stage.viewer {
                Some(_) => stage.layout,
                None => Layout::Console,
            },
            can_go_back: stage.pages.can_go_back(),
            can_go_forward: stage.pages.can_go_forward(),
        }
    }

    /// The address of the row in front: the page on its stage, or else its terminal's folder.
    pub fn address(&self) -> Address {
        let Some(tab) = self.book.ws.front() else {
            return Address::None;
        };
        let home = self.launch.ambient.home.to_string_lossy();
        let page = self
            .stages
            .rows
            .get(&tab)
            .filter(|stage| stage.layout.shows_stage())
            .and_then(|stage| stage.viewer)
            .and_then(|viewer| self.book.ws.viewers.get(&viewer));
        if let Some(file) = page {
            let file = file.as_str();
            return match temor_page(file) {
                Some(TemorUrl::Location(place)) => path_address(&place.path, &home, false),
                Some(page) => Address::Page(page),
                None => path_address(file, &home, true),
            };
        }
        self.row(tab)
            .and_then(|row| row.place)
            .map_or(Address::None, |place| path_address(&place, &home, false))
    }

    /// Show `layout` on the row in front. Without a page there is only the console.
    pub(super) fn set_layout(&mut self, layout: Layout) {
        let Some(tab) = self.book.ws.front() else {
            return;
        };
        let Some(stage) = self.stages.rows.get_mut(&tab) else {
            return;
        };
        let Some(viewer) = stage.viewer else { return };
        stage.layout = layout;
        // The keys stay where they can be seen.
        let Some(keys) = self.keyboard_pane() else {
            return;
        };
        let to = match keys == viewer {
            true if !layout.shows_stage() => self.viewer_unfocus_to(viewer),
            false if !layout.shows_console() => Some(viewer),
            _ => None,
        };
        if let Some(pane) = to {
            self.act(WorkspaceIn::Focus(pane));
        }
    }

    /// The terminal the keyboard goes back to from `viewer`.
    pub(super) fn viewer_unfocus_to(&self, viewer: PaneId) -> Option<PaneId> {
        match self.viewer_unfocus(viewer)? {
            WorkspaceIn::Focus(pane) => Some(pane),
            _ => None,
        }
    }

    /// Pop Out on the row in front: its page, when it is a file, leaves the stage (Back brings it
    /// back) for a window of its own. Answers the file. temor's own pages stay.
    pub(super) fn pop_out(&mut self) -> Option<PathBuf> {
        let tab = self.book.ws.front()?;
        let viewer = self.stages.rows.get(&tab)?.viewer?;
        let file = self.book.ws.viewers.get(&viewer)?.as_str().to_owned();
        if temor_page(&file).is_some() {
            return None;
        }
        self.act(WorkspaceIn::ClosePane(Some(viewer)));
        Some(PathBuf::from(file))
    }

    /// Back (`back`) or Forward on the row in front: its page before or after this one opens, or,
    /// Back from its first page, the page closes for the console.
    pub(super) fn walk_pages(&mut self, back: bool) {
        let Some(tab) = self.book.ws.front() else {
            return;
        };
        let Some(stage) = self.stages.rows.get_mut(&tab) else {
            return;
        };
        let page = match back {
            true => stage.pages.back().cloned(),
            false => stage.pages.forward().cloned(),
        };
        let viewer = stage.viewer;
        let Some(page) = page else {
            if let Some(viewer) = viewer.filter(|_| back) {
                self.stages.walking = true;
                self.act(WorkspaceIn::ClosePane(Some(viewer)));
                self.stages.walking = false;
            }
            return;
        };
        // From the terminal the page came from, or the row's first.
        let from = viewer
            .and_then(|viewer| self.viewer_unfocus_to(viewer))
            .or_else(|| {
                let panes = self.book.ws.tabs.tab(tab)?.tree.panes();
                panes.into_iter().find(|pane| !self.is_viewer(*pane))
            });
        self.stages.walking = true;
        self.act(WorkspaceIn::OpenViewer {
            file: ViewerRef(page.to_string_lossy().into_owned()),
            from,
        });
        self.stages.walking = false;
    }
}

impl HubRef {
    /// See [`Hub::stage_view`].
    pub fn stage_view(&self, tab: TabId) -> StageView {
        self.0.borrow().stage_view(tab)
    }

    /// See [`Hub::address`].
    pub fn address(&self) -> Address {
        self.0.borrow().address()
    }

    /// The file the page in the viewer `pane` is.
    pub fn page_file(&self, pane: PaneId) -> Option<String> {
        let hub = self.0.borrow();
        hub.book
            .ws
            .viewers
            .get(&pane)
            .map(|file| file.as_str().to_owned())
    }
}

impl Desk {
    /// Open temor's page `url` on the row in front (⌥⇧⌘Y opens the history).
    pub fn open_page(&self, url: TemorUrl) {
        self.settle(|hub| hub.open_page(url));
    }

    /// The history of every session.
    pub fn open_history(&self) {
        self.open_page(TemorUrl::History(HistoryScope::All));
    }

    /// The file `path` on the row in front's stage, as a folder's page opens it.
    pub fn open_file(&self, path: &Path) {
        let file = ViewerRef(path.to_string_lossy().into_owned());
        self.settle(|hub| hub.open_on_stage(file));
    }

    /// The page of the folder `path` on the row in front, as a crumb of the address opens it.
    pub fn open_folder(&self, path: &str) {
        self.open_page(folder_url(path));
    }

    /// The files that arrived, and those on their way to other computers.
    pub fn open_downloads(&self) {
        self.open_page(TemorUrl::Downloads);
    }

    /// Show `layout` on the row in front.
    pub fn set_layout(&self, layout: Layout) {
        self.settle(|hub| hub.set_layout(layout));
    }

    /// The page bar's Console button: the terminal beside the page, or hidden behind it.
    pub fn toggle_console(&self) {
        self.settle(|hub| {
            let now = hub
                .book
                .ws
                .front()
                .map(|tab| hub.stage_view(tab).layout)
                .unwrap_or_default();
            hub.set_layout(now.toggle_console());
        });
    }

    /// Pop Out on the row in front: its page, a file, opens in a window of its own.
    pub fn pop_out(&self) {
        let mut file = None;
        self.settle(|hub| file = hub.pop_out());
        if let Some(file) = file {
            crate::view::pop_out::open(PoppedOut {
                file,
                edge: self.hub.viewer_edge(),
            });
        }
    }

    /// Back on the row in front.
    pub fn go_back(&self) {
        self.settle(|hub| hub.walk_pages(true));
    }

    /// Forward on the row in front.
    pub fn go_forward(&self) {
        self.settle(|hub| hub.walk_pages(false));
    }
}
