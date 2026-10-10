//! Viewer panes in the hub (PLAN §10): a pane that shows a file instead of running a program.
//! The workspace says which panes are viewers and which file each shows; this keeps what the
//! workspace does not: where each came from (Esc goes back there), the files printed around it
//! (the arrows walk them), the pane's palette handle and the places the person left off.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyview_pane::{FilePath, NonEmpty, PaneEdge, PaneHandle, Resume, Sequence, SequenceOrigin};
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::ViewerRef;
use temor_core::workspace::{ViewerOpen, WorkspaceIn};

use super::model::{Hub, HubRef};
use crate::edge::viewer_workers::ViewerWorkers;
use crate::session::ViewAsk;

/// What the hub keeps for the viewer panes.
#[derive(Default)]
pub(super) struct ViewerHub {
    /// The pool and seams every viewer shares; made when the first viewer is drawn.
    edge: OnceCell<PaneEdge>,
    /// The files around the one being opened, for the viewer the workspace is about to announce.
    pending: Option<ViewAsk>,
    seats: BTreeMap<PaneId, Seat>,
    handles: BTreeMap<PaneId, PaneHandle>,
    /// Where the person left off in each file; kept for this run only.
    resumes: BTreeMap<PathBuf, Resume>,
}

/// One viewer pane's own state.
struct Seat {
    /// The file the pane was opened on. A different file replaces the pane's, so it stays put
    /// while the person walks the files around it.
    file: PathBuf,
    /// The terminal it was opened from.
    origin: Option<PaneId>,
    /// The files printed around `file`.
    around: Vec<PathBuf>,
}

/// What a viewer pane's view is given.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewerShow {
    pub file: FilePath,
    pub sequence: Option<Sequence>,
    pub edge: PaneEdge,
}

impl ViewerHub {
    fn edge(&self) -> PaneEdge {
        self.edge
            .get_or_init(|| PaneEdge::portable(Arc::new(ViewerWorkers::start())))
            .clone()
    }
}

impl Hub {
    /// A file was asked for from the terminal `from` (⌘-click, View): it opens beside it.
    pub fn open_viewer(&mut self, from: PaneId, ask: ViewAsk) {
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
        self.viewers.seats.insert(
            open.pane,
            Seat {
                file,
                origin: open.from,
                around,
            },
        );
    }

    /// A saved viewer pane comes back if its file is still there; if not, the pane is closed.
    pub(super) fn restore_viewer(&mut self, open: ViewerOpen) {
        let pane = open.pane;
        let there = Path::new(open.file.as_str()).is_file();
        self.viewer_opened(open);
        if !there {
            self.act(WorkspaceIn::ClosePane(Some(pane)));
            self.notices
                .push("A file you had open is gone, so its pane was closed.".to_owned());
        }
    }

    /// A viewer pane ended: its seat and handle go.
    pub(super) fn viewer_ended(&mut self, pane: PaneId) {
        self.viewers.seats.remove(&pane);
        self.viewers.handles.remove(&pane);
    }

    /// The request that gives the keyboard back from the viewer `pane`: to the terminal it came
    /// from, else to another terminal of its tab.
    pub fn viewer_unfocus(&self, pane: PaneId) -> Option<WorkspaceIn> {
        let tab = self.ws.tab_of(pane)?;
        let origin = self
            .viewers
            .seats
            .get(&pane)
            .and_then(|seat| seat.origin)
            .filter(|origin| self.ws.tab_of(*origin) == Some(tab));
        let other = || {
            self.ws
                .tabs
                .tab(tab)?
                .tree
                .panes()
                .into_iter()
                .find(|p| !self.ws.viewers.contains_key(p))
        };
        origin.or_else(other).map(WorkspaceIn::Focus)
    }

    /// The viewer now shows `path`: the session file keeps it, and the pane is left alone.
    pub fn viewer_shows(&mut self, pane: PaneId, path: &Path) {
        let file = ViewerRef(path.to_string_lossy().into_owned());
        if self.ws.viewers.get(&pane) != Some(&file) {
            self.act(WorkspaceIn::ViewerShows { pane, file });
        }
    }

    /// Keeps where the person left off in `file`.
    pub fn viewer_remember(&mut self, file: &Path, resume: Resume) {
        self.viewers.resumes.insert(file.to_path_buf(), resume);
    }

    /// The file the viewer `pane` shows now.
    pub(super) fn viewer_file_of(&self, pane: PaneId) -> Option<PathBuf> {
        self.ws
            .viewers
            .get(&pane)
            .map(|file| PathBuf::from(file.as_str()))
    }

    /// Whether `pane` is a viewer.
    pub fn is_viewer(&self, pane: PaneId) -> bool {
        self.ws.viewers.contains_key(&pane)
    }

    /// What the view of the viewer `pane` is given; none for a pane that is not one, or whose
    /// file has no absolute path.
    pub fn viewer_show(&self, pane: PaneId) -> Option<ViewerShow> {
        let seat = self.viewers.seats.get(&pane)?;
        let file = FilePath::new(&seat.file).ok()?;
        let entries: Vec<FilePath> = seat
            .around
            .iter()
            .filter_map(|path| FilePath::new(path).ok())
            .collect();
        let sequence = NonEmpty::from_vec(entries)
            .and_then(|e| Sequence::starting_at(e, &file, SequenceOrigin::Output).ok());
        Some(ViewerShow {
            file,
            sequence,
            edge: self.viewers.edge(),
        })
    }

    /// The viewer with the keyboard's palette handle, while there is one.
    pub fn keyboard_viewer(&self) -> Option<PaneHandle> {
        let pane = self.keyboard_pane()?;
        self.viewers.handles.get(&pane).copied()
    }

    /// The name a viewer pane is shown by: its file's name.
    pub(super) fn viewer_name(&self, pane: PaneId) -> Option<String> {
        let file = self.ws.viewers.get(&pane)?;
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

    /// See [`Hub::is_viewer`].
    pub fn is_viewer(&self, pane: PaneId) -> bool {
        self.0.borrow().is_viewer(pane)
    }

    /// The palette handle of the viewer pane that has the keyboard.
    pub fn keyboard_viewer(&self) -> Option<PaneHandle> {
        self.0.borrow().keyboard_viewer()
    }

    /// How the keymap routes `stroke` in a pane that is not a terminal: only a chord of temor's is
    /// an action.
    pub fn route_key(&self, stroke: crate::input::Stroke) -> crate::input::Routed {
        let hub = self.0.borrow();
        crate::input::route(stroke, &hub.launch.keymap, crate::input::FindBar::Closed)
    }

    /// The viewer's view hands over its palette handle while it is drawn.
    pub fn viewer_handle(&self, pane: PaneId, handle: PaneHandle) {
        self.0.borrow_mut().viewers.handles.insert(pane, handle);
    }

    /// The viewer's view is going: its handle is no longer good.
    pub fn viewer_handle_gone(&self, pane: PaneId) {
        self.0.borrow_mut().viewers.handles.remove(&pane);
    }
}
