//! Files dropped on the pane (PLAN §9). Nothing is known about the program there, so the paths
//! are typed into the prompt the way the shell quotes them, as a paste: the person's Enter is
//! what runs anything. temor-core decides (`drop_action`); this carries it out.

use std::path::PathBuf;

use temor_client::model::shelf::{
    DropAction, Dropped, PaneSite, Quoting, ReceiveSupport, drop_action,
};
use temor_panes::pane::paste;
use temor_panes::vt::ViewportScroll;

use super::Session;
use crate::words::notices;

impl Session {
    /// Says how the pane's shell quotes a word (from the shell the pane runs).
    pub fn set_quoting(&mut self, quoting: Quoting) {
        self.core().quoting = quoting;
    }

    /// Says whether the pane's shell runs on this computer or on another.
    pub fn set_site(&mut self, site: PaneSite) {
        self.core().site = site;
    }

    /// Whether the pane's shell runs on this computer or on another.
    pub fn site(&self) -> PaneSite {
        self.core().site
    }

    /// Carries out a drop: the quoted paths are pasted, or the person is told why not.
    pub fn drop_files(&mut self, paths: &[PathBuf]) {
        let words: Vec<String> = paths
            .iter()
            .filter_map(|path| path.to_str().map(str::to_owned))
            .collect();
        if words.len() != paths.len() {
            self.notice = Some("That file's name can't be typed into a terminal".to_owned());
            return;
        }
        let (site, quoting) = {
            let core = self.core();
            (core.site, core.quoting)
        };
        let dropped = Dropped {
            paths: words,
            // No program here has said it takes files; the paths are typed instead.
            support: ReceiveSupport::Unknown,
            site,
            quoting,
        };
        match drop_action(&dropped) {
            DropAction::Paste(text) => {
                if self.composer_takes(&text) {
                    return;
                }
                let mode = self.pane.with_term(|term| term.paste_mode());
                self.pane.send(&paste::encode(&text, mode));
                self.pane
                    .with_term(|term| term.scroll(ViewportScroll::Bottom));
            }
            DropAction::Send(_) => {
                self.notice = Some("temor can't send files to this program yet".to_owned());
            }
            DropAction::Tell(words) => self.notice = Some(words.to_owned()),
            DropAction::Nothing => {}
        }
    }

    /// Puts a file's path on the clipboard.
    pub fn copy_path(&mut self, path: &str) {
        if !self.put_on_clipboard(path) {
            self.notice = Some(notices::PATH_NOT_COPIED.to_owned());
        }
    }
}
