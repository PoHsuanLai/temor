//! The desktop's hands for a chosen plugin row: the clipboard, the link opener and the file opener
//! the hub already has. Tests use a fake `Hands` instead; the desktop is never touched by them.

use std::path::Path;

use super::effects::Hands;
use crate::platform::{FileShell, Opener, open_link};
use crate::session::Clipboard;
use crate::view::launch::DesktopClipboard;
use crate::words::notices;

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
            .map_err(|_| notices::TEXT_NOT_COPIED.to_owned())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        open_link(self.links, url).map_err(str::to_owned)
    }

    fn open_file(&self, path: &str) -> Result<(), String> {
        self.files
            .open(Path::new(path))
            .map_err(|_| notices::FILE_NOT_OPENED.to_owned())
    }
}
