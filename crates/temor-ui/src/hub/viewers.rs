//! Viewer panes in the hub (PLAN §10): a pane that shows a file instead of running a program.
//! The workspace says which panes are viewers and which file each shows; this keeps what the
//! workspace does not: where each came from (Esc goes back there), the files printed around it
//! (the arrows walk them), and the pane's palette handle. Where the person left off in a file is
//! the pane's: anyview's one remembering policy writes it to the store.

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyview_machines::StageFamily;
use anyview_pane::{FilePath, NonEmpty, PaneEdge, PaneHandle, Sequence, SequenceOrigin};
use temor_client::model::shelf::PaneSite;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::vocab::text::ViewerRef;
use temor_client::model::workspace::{ViewerOpen, WorkspaceIn};

use super::model::{Hub, HubRef};
use crate::bar::Finding;
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
    /// The viewers whose file has no stage to show it (a kind the viewer only describes).
    describes_only: BTreeSet<PaneId>,
    /// The viewers whose file could not be opened (anyview's failure card is up), with that file,
    /// which another app may still open. A later open of any file clears it.
    not_opened: BTreeMap<PaneId, anyview_pane::FilePath>,
    /// Whether the bar that is about to open starts as a search of the viewer's file (⌘F).
    find_wanted: Finding,
    /// The files fetched from other computers, and the copies the viewers show.
    pub(super) host: super::viewer_host::HostCopies,
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
        self.host_copy_shown(open.pane, &file);
        let around = match self.viewers.pending.take() {
            Some(ask) if ask.file == file => ask.around,
            _ => Vec::new(),
        };
        self.stage_opened(open.tab, open.pane, file.clone());
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

    /// A viewer pane ended: its seat and handle go.
    pub(super) fn viewer_ended(&mut self, pane: PaneId) {
        self.stage_closed(pane);
        self.viewers.seats.remove(&pane);
        self.viewers.handles.remove(&pane);
        self.viewers.describes_only.remove(&pane);
        self.viewers.not_opened.remove(&pane);
        self.host_copy_ended(pane);
    }

    /// The request that gives the keyboard back from the viewer `pane`: to the terminal it came
    /// from, else to another terminal of its tab.
    pub fn viewer_unfocus(&self, pane: PaneId) -> Option<WorkspaceIn> {
        let tab = self.book.ws.tab_of(pane)?;
        let origin = self
            .viewers
            .seats
            .get(&pane)
            .and_then(|seat| seat.origin)
            .filter(|origin| self.book.ws.tab_of(*origin) == Some(tab));
        let other = || {
            self.book
                .ws
                .tabs
                .tab(tab)?
                .tree
                .panes()
                .into_iter()
                .find(|p| !self.book.ws.viewers.contains_key(p))
        };
        origin.or_else(other).map(WorkspaceIn::Focus)
    }

    /// The viewer now shows `path`: the session file keeps it, and the pane is left alone.
    pub fn viewer_shows(&mut self, pane: PaneId, path: &Path) {
        let file = ViewerRef(path.to_string_lossy().into_owned());
        if self.book.ws.viewers.get(&pane) != Some(&file) {
            self.act(WorkspaceIn::ViewerShows { pane, file });
        }
    }

    /// The viewer `pane` opened a file, and whether the viewer can show it or only describe it.
    pub fn viewer_family(&mut self, pane: PaneId, family: StageFamily) {
        self.viewers.not_opened.remove(&pane);
        match family {
            StageFamily::PeekOnly => self.viewers.describes_only.insert(pane),
            _ => self.viewers.describes_only.remove(&pane),
        };
    }

    /// Whether the viewer `pane` can only describe its file, which another app may open.
    pub fn viewer_describes_only(&self, pane: PaneId) -> bool {
        self.viewers.describes_only.contains(&pane)
    }

    /// The viewer `pane` could not open `file`.
    pub fn viewer_not_opened(&mut self, pane: PaneId, file: anyview_pane::FilePath) {
        self.viewers.describes_only.remove(&pane);
        self.viewers.not_opened.insert(pane, file);
    }

    /// The file the viewer `pane` could not open, if its last open failed.
    pub fn viewer_failed_file(&self, pane: PaneId) -> Option<&anyview_pane::FilePath> {
        self.viewers.not_opened.get(&pane)
    }

    /// Whether `pane` is a viewer.
    pub fn is_viewer(&self, pane: PaneId) -> bool {
        self.book.ws.viewers.contains_key(&pane)
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
            edge: self.viewers.edge(self.viewer_store.as_deref()),
        })
    }

    /// The viewer with the keyboard's palette handle, while there is one.
    pub fn keyboard_viewer(&self) -> Option<PaneHandle> {
        let pane = self.keyboard_pane()?;
        self.viewers.handles.get(&pane).copied()
    }

    /// The name a viewer pane is shown by: its file's name.
    pub(super) fn viewer_name(&self, pane: PaneId) -> Option<String> {
        let file = self.book.ws.viewers.get(&pane)?;
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
            hub.viewers.find_wanted = Finding::On;
        }
    }

    /// What the bar that just opened starts as; asking clears it, so the next ⌘T opens plain.
    pub fn take_finding_wanted(&self) -> Finding {
        std::mem::take(&mut self.0.borrow_mut().viewers.find_wanted)
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
