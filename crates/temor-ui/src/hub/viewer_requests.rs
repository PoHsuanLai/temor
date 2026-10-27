//! What a viewer pane asks of the window (PLAN §10): to be closed, to give the keyboard back, to
//! hand a file to the desktop. Anything it asks that the window does not do yet is declined with
//! a short note rather than ignored.

use anyview_pane::{FileRequest, PaneRequest};
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::WorkspaceIn;

use super::desk::Desk;
use super::model::Hub;
use crate::platform::open_link;

impl Desk {
    /// Answers one request of the viewer pane `pane`.
    pub fn viewer_request(&self, pane: PaneId, request: PaneRequest) {
        match request {
            // The pane takes the room the split gives it; a size it would like is only a hint.
            PaneRequest::SizeHint(_) | PaneRequest::Detach(_) => {}
            PaneRequest::ClosePane => self.act(WorkspaceIn::ClosePane(Some(pane))),
            PaneRequest::Unfocus => {
                let back = self.hub.0.borrow().viewer_unfocus(pane);
                if let Some(input) = back {
                    self.act(input);
                }
            }
            PaneRequest::OpenElsewhere(files) => {
                self.settle(|hub| hub.viewer_open_elsewhere(&files));
            }
            PaneRequest::Opened(opened) => {
                let path = opened.source.path().as_path().to_path_buf();
                let family = opened.family;
                self.settle(|hub| {
                    hub.viewer_shows(pane, &path);
                    hub.viewer_family(pane, family);
                });
            }
            // Where the person left off: with a store the pane keeps it itself, by anyview's one
            // remembering policy (`Remembering`), and never asks; a run without a store keeps
            // nothing, so there is nothing to time or write here.
            PaneRequest::Remember(_) => {}
            // The files are on this computer and nothing changes them behind the pane's back
            // that temor hears of yet.
            PaneRequest::Watch(_) | PaneRequest::Unwatch => {}
            PaneRequest::File(change) => self.settle(|hub| hub.viewer_file_request(change)),
            // The pane shows why on its own failure card; the window offers another app.
            PaneRequest::NotOpened(failed) => {
                self.settle(|hub| hub.viewer_not_opened(pane, failed.file));
            }
            // `PaneRequest` is non-exhaustive: a request this temor does not know is ignored.
            _ => {}
        }
    }
}

impl Desk {
    /// "Open in another app" on a viewer that can only describe its file: the desktop opens the
    /// file with the program it chose for it.
    pub fn viewer_open_in_app(&self, pane: PaneId) {
        self.settle(|hub| hub.viewer_file_in_app(pane));
    }
}

impl Hub {
    /// The desktop opens the file the viewer `pane` shows with the program it chose for it.
    fn viewer_file_in_app(&mut self, pane: PaneId) {
        // A file that could not be opened was never recorded as shown: it is the one to hand on.
        let file = self.viewer_failed_file(pane).cloned().or_else(|| {
            self.ws.viewers.get(&pane).and_then(|file| {
                anyview_pane::FilePath::new(super::stage::page_path(file.as_str())).ok()
            })
        });
        if let Some(file) = file {
            self.viewer_open_elsewhere(&[file]);
        }
    }

    /// The desktop opens `files` with the program it chose for them.
    fn viewer_open_elsewhere(&mut self, files: &[anyview_pane::FilePath]) {
        if files.is_empty() {
            self.notices
                .push("Choosing a file to open is not available here yet.".to_owned());
        }
        for file in files {
            if self.ssh.file_opener.open(file.as_path()).is_err() {
                self.notices
                    .push("That file could not be opened.".to_owned());
            }
        }
    }

    /// A change or action on the shown file: the web and mail addresses in it open and the
    /// file shows in the file manager; everything that would change the file is declined.
    fn viewer_file_request(&mut self, request: FileRequest) {
        let note = match request {
            FileRequest::OpenUri(address) => open_link(&*self.ssh.opener, &address)
                .err()
                .map(str::to_owned),
            FileRequest::Reveal(file) => self
                .ssh
                .file_opener
                .reveal(file.as_path())
                .err()
                .map(|_| "That folder could not be shown.".to_owned()),
            _ => Some("Changing files from here is not available yet.".to_owned()),
        };
        self.notices.extend(note);
    }
}
