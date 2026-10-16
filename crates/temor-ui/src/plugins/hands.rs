//! The desktop's hands for a chosen plugin row: the clipboard, the link opener and the file opener
//! the hub already has. Tests use a fake `Hands` instead; the desktop is never touched by them.

use std::path::Path;

use temor_pane::text::{LinkVerdict, check_link};

use super::effects::Hands;
use crate::platform::{FileShell, Opener};
use crate::session::Clipboard;
use crate::view::launch::DesktopClipboard;

/// The desktop's hands: the clipboard, the link opener and the file opener the hub already has.
pub(crate) struct DesktopHands<'a> {
    pub(crate) links: &'a dyn Opener,
    pub(crate) files: &'a dyn FileShell,
}

impl std::fmt::Debug for DesktopHands<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DesktopHands")
    }
}

impl Hands for DesktopHands<'_> {
    fn copy(&self, text: &str) -> Result<(), String> {
        DesktopClipboard
            .write(text)
            .map_err(|_| "That could not be copied.".to_owned())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        match check_link(url) {
            LinkVerdict::Open(link) => self
                .links
                .open(&link)
                .map_err(|_| "That link could not be opened.".to_owned()),
            LinkVerdict::Refused(_) => Err("Only web and email links open from here.".to_owned()),
        }
    }

    fn open_file(&self, path: &str) -> Result<(), String> {
        self.files
            .open(Path::new(path))
            .map_err(|_| "That file could not be opened.".to_owned())
    }
}
