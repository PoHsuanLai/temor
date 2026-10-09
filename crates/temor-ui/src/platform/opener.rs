//! Opening a link in the desktop's own handler (PLAN §6: ⌘-click on a hyperlink).

use thiserror::Error;

/// Why a link could not be opened.
#[derive(Debug, Error)]
pub enum OpenError {
    /// This system has no opener yet.
    #[error("opening links is not available on this system yet")]
    Unsupported,
    /// The opener ran and failed, or could not start.
    #[error("the link could not be opened: {0}")]
    Failed(#[source] std::io::Error),
}

/// Hands a link to the desktop. Implemented by the platform's own handler.
pub trait Opener {
    fn open(&self, target: &str) -> Result<(), OpenError>;
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Command, Stdio};

    use super::{OpenError, Opener};

    /// `xdg-open`, the freedesktop launcher every Linux desktop ships.
    #[derive(Debug, Default)]
    pub struct XdgOpen;

    impl Opener for XdgOpen {
        fn open(&self, target: &str) -> Result<(), OpenError> {
            // The child is detached: nothing waits for the browser to exit.
            Command::new("xdg-open")
                .arg(target)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(drop)
                .map_err(OpenError::Failed)
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod other {
    use super::{OpenError, Opener};

    /// Stub until the platform's opener is written.
    // TODO(platform:macos): NSWorkspace open; TODO(platform:windows): ShellExecute.
    #[derive(Debug, Default)]
    pub struct Unsupported;

    impl Opener for Unsupported {
        fn open(&self, _target: &str) -> Result<(), OpenError> {
            Err(OpenError::Unsupported)
        }
    }
}

/// The platform's opener.
pub fn default_opener() -> Box<dyn Opener> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::XdgOpen)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(other::Unsupported)
    }
}
