//! The one file opener (PLAN §9): opening a file (a saved download, a local copy of a host's file
//! the person asked to edit) and showing it in the file manager. The path is one temor made.
//! What differs by system is temor-desktop's (`SystemOpen`).

use std::path::Path;

use temor_client::desktop::SystemOpen;

use super::opener::OpenError;

/// What the downloads shelf asks of the desktop.
pub trait FileShell {
    /// Opens the file with the program the desktop chose for it.
    fn open(&self, path: &Path) -> Result<(), OpenError>;
    /// Shows the file in the file manager.
    fn reveal(&self, path: &Path) -> Result<(), OpenError>;
}

/// The desktop's opener (`temor_client::desktop::SystemOpen`).
#[derive(Debug, Default)]
pub struct DesktopFiles;

impl FileShell for DesktopFiles {
    fn open(&self, path: &Path) -> Result<(), OpenError> {
        SystemOpen::open(path).map_err(OpenError::from_io)
    }

    fn reveal(&self, path: &Path) -> Result<(), OpenError> {
        SystemOpen::reveal(path).map_err(OpenError::from_io)
    }
}

/// The platform's file opener.
pub fn default_file_shell() -> Box<dyn FileShell> {
    Box::new(DesktopFiles)
}
