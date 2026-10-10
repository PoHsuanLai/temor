//! What a viewer pane asks of the window (PLAN §10): to be closed, to give the keyboard back, to
//! hand a file to the desktop. Anything it asks that the window does not do yet is declined with
//! a short note rather than ignored.

use anyview_pane::{FileRequest, PaneRequest};
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::WorkspaceIn;
use temor_pane::text::{LinkVerdict, check_link};

use super::desk::Desk;
use super::model::Hub;

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
                self.settle(|hub| hub.viewer_shows(pane, &path));
            }
            PaneRequest::Remember(resume) => {
                let shown = self.hub.0.borrow().viewer_file_of(pane);
                if let Some(file) = shown {
                    self.hub.0.borrow_mut().viewer_remember(&file, resume);
                }
            }
            // The files are on this computer and nothing changes them behind the pane's back
            // that temor hears of yet.
            PaneRequest::Watch(_) | PaneRequest::Unwatch => {}
            PaneRequest::File(change) => self.settle(|hub| hub.viewer_file_request(change)),
        }
    }
}

impl Hub {
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
            FileRequest::OpenUri(address) => match check_link(&address) {
                LinkVerdict::Open(link) => self
                    .ssh
                    .opener
                    .open(&link)
                    .err()
                    .map(|_| "That link could not be opened.".to_owned()),
                LinkVerdict::Refused(_) => {
                    Some("Only web and email links open from here.".to_owned())
                }
            },
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
