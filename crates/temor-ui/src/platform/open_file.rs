//! Opening a local file in the desktop's own program for it (a copy of a host's file the person
//! asked to edit). The path is one temor made, never text from a program.

use std::path::Path;

use super::OpenError;

/// Hands a local file to the desktop. Implemented by the platform's own handler.
pub trait FileOpener {
    fn open_file(&self, path: &Path) -> Result<(), OpenError>;
}

#[cfg(target_os = "linux")]
mod linux {
    use std::path::Path;
    use std::process::{Command, Stdio};

    use super::{FileOpener, OpenError};

    /// `xdg-open`, as for links.
    #[derive(Debug, Default)]
    pub struct XdgOpenFile;

    impl FileOpener for XdgOpenFile {
        fn open_file(&self, path: &Path) -> Result<(), OpenError> {
            let mut child = Command::new("xdg-open")
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(OpenError::Failed)?;
            // Waited for in the background so it never stays a zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod other {
    use std::path::Path;

    use super::{FileOpener, OpenError};

    /// Stub until the platform's opener is written.
    // TODO(platform:macos): NSWorkspace open; TODO(platform:windows): ShellExecute.
    #[derive(Debug, Default)]
    pub struct UnsupportedFile;

    impl FileOpener for UnsupportedFile {
        fn open_file(&self, _path: &Path) -> Result<(), OpenError> {
            Err(OpenError::Unsupported)
        }
    }
}

/// The platform's file opener.
pub fn default_file_opener() -> Box<dyn FileOpener> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::XdgOpenFile)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(other::UnsupportedFile)
    }
}
