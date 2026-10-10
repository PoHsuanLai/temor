//! The one file opener (PLAN §9): opening a file (a saved download, a local copy of a host's file
//! the person asked to edit) and showing it in the file manager. The path is one temor made.
//! What differs by system lives here; Linux hands the path to the desktop's opener, other
//! systems have a stub.

use std::path::Path;

use super::opener::OpenError;

/// What the downloads shelf asks of the desktop.
pub trait FileShell {
    /// Opens the file with the program the desktop chose for it.
    fn open(&self, path: &Path) -> Result<(), OpenError>;
    /// Shows the file in the file manager.
    fn reveal(&self, path: &Path) -> Result<(), OpenError>;
}

#[cfg(target_os = "linux")]
mod linux {
    use std::path::Path;
    use std::process::{Command, Stdio};

    use super::{FileShell, OpenError};

    /// `xdg-open`, for the file and for its folder.
    #[derive(Debug, Default)]
    pub struct XdgFiles;

    impl XdgFiles {
        fn run(path: &Path) -> Result<(), OpenError> {
            let mut child = Command::new("xdg-open")
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(OpenError::Failed)?;
            // The opener is the edge's own child: wait for it off the window's thread.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
    }

    impl FileShell for XdgFiles {
        fn open(&self, path: &Path) -> Result<(), OpenError> {
            Self::run(path)
        }

        // TODO(platform:linux): org.freedesktop.FileManager1.ShowItems selects the file; the
        // folder is the fallback every desktop has.
        fn reveal(&self, path: &Path) -> Result<(), OpenError> {
            Self::run(path.parent().unwrap_or(path))
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod other {
    use std::path::Path;

    use super::{FileShell, OpenError};

    /// Stub until the platform's file opener is written.
    // TODO(platform:macos): NSWorkspace open and activateFileViewerSelecting;
    // TODO(platform:windows): ShellExecute and `explorer /select`.
    #[derive(Debug, Default)]
    pub struct Unsupported;

    impl FileShell for Unsupported {
        fn open(&self, _path: &Path) -> Result<(), OpenError> {
            Err(OpenError::Unsupported)
        }

        fn reveal(&self, _path: &Path) -> Result<(), OpenError> {
            Err(OpenError::Unsupported)
        }
    }
}

/// The platform's file opener.
pub fn default_file_shell() -> Box<dyn FileShell> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::XdgFiles)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(other::Unsupported)
    }
}
