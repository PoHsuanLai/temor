//! Viewer panes in the hub (PLAN §10): a pane that shows a file instead of running a program.
//! The workspace says which panes are viewers and which file each shows; this keeps what the
//! workspace does not: where each came from (Esc goes back there), the files printed around it
//! (the arrows walk them), and the pane's palette handle. Where the person left off in a file is
//! the pane's: anyview's one remembering policy writes it to the store.

use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyview_machines::StageFamily;
use anyview_pane::{FilePath, NonEmpty, PaneEdge, PaneHandle, Sequence, SequenceOrigin};
use temor_client::model::shelf::PaneSite;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::vocab::text::ViewerRef;
use temor_client::model::workspace::{ViewerOpen, WorkspaceIn};

use super::model::{Hub, HubRef};
use temor_pane_view::session::ViewAsk;
use temor_ui_base::edge::viewer_workers::ViewerWorkers;

/// What the viewer panes share. What each keeps of its own is in its record ([`PaneViewer`]).
#[derive(Default)]
pub(super) struct ViewerHub {
    /// The pool and seams every viewer shares; made when the first viewer is drawn.
    edge: OnceCell<PaneEdge>,
    /// The files around the one being opened, for the viewer the workspace is about to announce.
    pending: Option<ViewAsk>,
    /// Whether the bar that is about to open starts as a search of the viewer's file (⌘F).
    find_wanted: bool,
    /// The files fetched from other computers, and the copies the viewers show.
    pub(super) host: super::viewer_host::HostCopies,
}

/// One viewer pane's own state, kept in its record.
pub(super) struct PaneViewer {
    /// The file the pane was opened on. A different file replaces the pane's, so it stays put
    /// while the person walks the files around it.
    file: PathBuf,
    /// The terminal it was opened from.
    origin: Option<PaneId>,
    /// The files printed around `file`.
    around: Vec<PathBuf>,
    /// Its palette handle, while its view is drawn.
    handle: Option<PaneHandle>,
    /// How its last open went.
    opened: Opened,
    /// The copy of a host's file it shows, removed when it ends.
    host_copy: Option<PathBuf>,
}

/// How a viewer's last open went.
#[derive(Debug, Clone, Default)]
enum Opened {
    /// It shows its file (or has not said yet).
    #[default]
    Shows,
    /// Its file has no stage to show it: a kind the viewer only describes.
    DescribesOnly,
    /// Its file could not be opened (anyview's failure card is up); another app may still open
    /// it. A later open of any file clears it.
    Failed(FilePath),
}

/// What a viewer pane's view is given.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewerShow {
    pub file: FilePath,
    pub sequence: Option<Sequence>,
    pub edge: PaneEdge,
}

impl ViewerHub {
    /// The shared edge. With a store (a run that keeps its state), the viewer keeps where each
    /// file was left there, shared with anyview's own windows; without one it remembers for this
    /// run only.
    fn edge(&self, store: Option<&std::path::Path>) -> PaneEdge {
        self.edge
            .get_or_init(|| {
                let edge = PaneEdge::portable(Arc::new(ViewerWorkers::start()));
                match store {
                    Some(root) => edge.with_store(root),
                    None => edge,
                }
            })
            .clone()
    }
}

impl Hub {
    /// A file was asked for from the terminal `from` (⌘-click, View): it opens beside it.
    pub fn open_viewer(&mut self, from: PaneId, ask: ViewAsk) {
        // A folder here is a page of its own.
        if ask.site == PaneSite::Local && ask.file.is_dir() {
            let folder = super::stage::folder_url(&ask.file.to_string_lossy());
            self.act(WorkspaceIn::OpenViewer {
                file: ViewerRef(folder.to_string()),
                from: Some(from),
            });
            return;
        }
        let file = ViewerRef(ask.file.to_string_lossy().into_owned());
        self.viewers.pending = Some(ask);
        self.act(WorkspaceIn::OpenViewer {
            file,
            from: Some(from),
        });
        self.viewers.pending = None;
    }

    /// Holds the files around the one the workspace is about to announce (tests announce by hand).
    #[cfg(test)]
    pub(super) fn viewers_ask(&mut self, ask: ViewAsk) {
        self.viewers.pending = Some(ask);
    }

    /// The workspace announced a viewer pane: it has no program, only a seat.
    pub(super) fn viewer_opened(&mut self, open: ViewerOpen) {
        let file = PathBuf::from(open.file.as_str());
        let around = match self.viewers.pending.take() {
            Some(ask) if ask.file == file => ask.around,
            _ => Vec::new(),
        };
        self.stage_opened(open.tab, open.pane, file.clone());
        let host_copy = self.is_host_copy(&file).then(|| file.clone());
        let record = self.panes.record(open.pane);
        // A tab's viewer shows the next file in place: its view, and so its handle, stay.
        let before = record.viewer.take();
        let (handle, opened) = before.as_ref().map_or((None, Opened::Shows), |before| {
            (before.handle, before.opened.clone())
        });
        record.viewer = Some(PaneViewer {
            file,
            origin: open.from,
            around,
            handle,
            opened,
            host_copy,
        });
        // The copy it showed before goes, unless it shows it still.
        if let Some(copy) = before.and_then(|before| before.host_copy)
            && record.viewer.as_ref().and_then(|v| v.host_copy.as_ref()) != Some(&copy)
        {
            super::ssh_edit::forget_copy(&copy);
        }
    }

    /// A saved viewer pane comes back if its file is still there; if not, the pane is closed.
    pub(super) fn restore_viewer(&mut self, open: ViewerOpen) {
        let pane = open.pane;
        // temor's own pages are always there.
        if super::stage::temor_page(open.file.as_str()).is_some() {
            return self.viewer_opened(open);
        }
        let file = PathBuf::from(open.file.as_str());
        // A copy of a file on another computer does not outlive the window that fetched it.
        if self.is_host_copy(&file) {
            super::ssh_edit::forget_copy(&file);
            self.viewer_opened(open);
            self.act(WorkspaceIn::ClosePane(Some(pane)));
            self.notices.push(
                "A file from another computer was closed with the window; ⌘-click it to get it again."
                    .to_owned(),
            );
            return;
        }
        let there = file.is_file();
        self.viewer_opened(open);
        if !there {
            self.act(WorkspaceIn::ClosePane(Some(pane)));
            self.notices
                .push("A file you had open is gone, so its pane was closed.".to_owned());
        }
    }

    /// A viewer pane ended: what it kept goes, and the copy of a host's file it showed.
    pub(super) fn viewer_ended(&mut self, pane: PaneId) {
        self.stage_closed(pane);
        let ended = self
            .panes
            .get_mut(pane)
            .and_then(|record| record.viewer.take());
        if let Some(copy) = ended.and_then(|viewer| viewer.host_copy) {
            super::ssh_edit::forget_copy(&copy);
        }
    }

    /// What the viewer `pane` keeps, while it is one.
    fn viewer(&self, pane: PaneId) -> Option<&PaneViewer> {
        self.panes.get(pane)?.viewer.as_ref()
    }

    /// What the viewer `pane` keeps, to change, while it is one.
    fn viewer_mut(&mut self, pane: PaneId) -> Option<&mut PaneViewer> {
        self.panes.get_mut(pane)?.viewer.as_mut()
    }

    /// The request that gives the keyboard back from the viewer `pane`: to the terminal it came
    /// from, else to another terminal of its tab.
    pub fn viewer_unfocus(&self, pane: PaneId) -> Option<WorkspaceIn> {
        let tab = self.book.ws().tab_of(pane)?;
        let origin = self
            .viewer(pane)
            .and_then(|viewer| viewer.origin)
            .filter(|origin| self.book.ws().tab_of(*origin) == Some(tab));
        let other = || {
            self.book
                .ws()
                .tabs
                .tab(tab)?
                .tree
                .panes()
                .into_iter()
                .find(|p| !self.book.ws().viewers.contains_key(p))
        };
        origin.or_else(other).map(WorkspaceIn::Focus)
    }

    /// The viewer now shows `path`: the session file keeps it, and the pane is left alone.
    pub fn viewer_shows(&mut self, pane: PaneId, path: &Path) {
        let file = ViewerRef(path.to_string_lossy().into_owned());
        if self.book.ws().viewers.get(&pane) != Some(&file) {
            self.act(WorkspaceIn::ViewerShows { pane, file });
        }
    }

    /// The viewer `pane` opened a file, and whether the viewer can show it or only describe it.
    pub fn viewer_family(&mut self, pane: PaneId, family: StageFamily) {
        if let Some(viewer) = self.viewer_mut(pane) {
            viewer.opened = match family {
                StageFamily::PeekOnly => Opened::DescribesOnly,
                _ => Opened::Shows,
            };
        }
    }

    /// Whether the viewer `pane` can only describe its file, which another app may open.
    pub fn viewer_describes_only(&self, pane: PaneId) -> bool {
        self.viewer(pane)
            .is_some_and(|viewer| matches!(viewer.opened, Opened::DescribesOnly))
    }

    /// The viewer `pane` could not open `file`.
    pub fn viewer_not_opened(&mut self, pane: PaneId, file: anyview_pane::FilePath) {
        if let Some(viewer) = self.viewer_mut(pane) {
            viewer.opened = Opened::Failed(file);
        }
    }

    /// The file the viewer `pane` could not open, if its last open failed.
    pub fn viewer_failed_file(&self, pane: PaneId) -> Option<&anyview_pane::FilePath> {
        match &self.viewer(pane)?.opened {
            Opened::Failed(file) => Some(file),
            Opened::Shows | Opened::DescribesOnly => None,
        }
    }

    /// Whether `pane` is a viewer.
    pub fn is_viewer(&self, pane: PaneId) -> bool {
        self.book.ws().viewers.contains_key(&pane)
    }

    /// What the view of the viewer `pane` is given; none for a pane that is not one, or whose
    /// file has no absolute path.
    pub fn viewer_show(&self, pane: PaneId) -> Option<ViewerShow> {
        let viewer = self.viewer(pane)?;
        let file = FilePath::new(&viewer.file).ok()?;
        let entries: Vec<FilePath> = viewer
            .around
            .iter()
            .filter_map(|path| FilePath::new(path).ok())
            .collect();
        let sequence = NonEmpty::from_vec(entries)
            .and_then(|e| Sequence::starting_at(e, &file, SequenceOrigin::Output).ok());
        Some(ViewerShow {
            file,
            sequence,
            edge: self.viewers.edge(self.viewer_store.as_deref()),
        })
    }

    /// The viewer with the keyboard's palette handle, while there is one.
    pub fn keyboard_viewer(&self) -> Option<PaneHandle> {
        let pane = self.keyboard_pane()?;
        self.viewer(pane)?.handle
    }

    /// The name a viewer pane is shown by: its file's name.
    pub(super) fn viewer_name(&self, pane: PaneId) -> Option<String> {
        let file = self.book.ws().viewers.get(&pane)?;
        if let Some(page) = super::stage::temor_page(file.as_str()) {
            return Some(super::stage::temor_page_name(&page));
        }
        Path::new(file.as_str())
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }
}

impl HubRef {
    /// See [`Hub::open_viewer`].
    pub fn open_viewer(&self, from: PaneId, ask: ViewAsk) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.open_viewer(from, ask);
        std::mem::take(&mut hub.notices)
    }

    /// See [`Hub::viewer_show`].
    pub fn viewer_show(&self, pane: PaneId) -> Option<ViewerShow> {
        self.0.borrow().viewer_show(pane)
    }

    /// The edge every viewer runs over, a popped-out page's too.
    pub fn viewer_edge(&self) -> PaneEdge {
        let hub = self.0.borrow();
        hub.viewers.edge(hub.viewer_store.as_deref())
    }

    /// See [`Hub::viewer_describes_only`].
    pub fn viewer_describes_only(&self, pane: PaneId) -> bool {
        self.0.borrow().viewer_describes_only(pane)
    }

    /// Whether the viewer `pane`'s last open failed (see [`Hub::viewer_failed_file`]).
    pub fn viewer_not_opened(&self, pane: PaneId) -> bool {
        self.0.borrow().viewer_failed_file(pane).is_some()
    }

    /// See [`Hub::is_viewer`].
    pub fn is_viewer(&self, pane: PaneId) -> bool {
        self.0.borrow().is_viewer(pane)
    }

    /// The palette handle of the viewer pane that has the keyboard.
    pub fn keyboard_viewer(&self) -> Option<PaneHandle> {
        self.0.borrow().keyboard_viewer()
    }

    /// ⌘F in a viewer pane: the bar about to open starts as a search of the file ("In This File").
    /// Nothing is asked when no viewer has the keyboard.
    pub fn want_find_in_viewer(&self) {
        let mut hub = self.0.borrow_mut();
        if hub.keyboard_viewer().is_some() {
            hub.viewers.find_wanted = true;
        }
    }

    /// What the bar that just opened starts as; asking clears it, so the next ⌘T opens plain.
    pub fn take_finding_wanted(&self) -> bool {
        std::mem::take(&mut self.0.borrow_mut().viewers.find_wanted)
    }

    /// How the keymap routes `stroke` in a pane that is not a terminal: only a chord of temor's is
    /// an action.
    pub fn route_key(
        &self,
        stroke: temor_pane_view::input::Stroke,
    ) -> temor_pane_view::input::Routed {
        let hub = self.0.borrow();
        temor_pane_view::input::route(stroke, &hub.launch.keymap, false)
    }

    /// The viewer's view hands over its palette handle while it is drawn.
    pub fn viewer_handle(&self, pane: PaneId, handle: PaneHandle) {
        if let Some(viewer) = self.0.borrow_mut().viewer_mut(pane) {
            viewer.handle = Some(handle);
        }
    }

    /// The viewer's view is going: its handle is no longer good.
    pub fn viewer_handle_gone(&self, pane: PaneId) {
        if let Some(viewer) = self.0.borrow_mut().viewer_mut(pane) {
            viewer.handle = None;
        }
    }
}
