//! Opening a link in the desktop's own handler (PLAN §6: ⌘-click on a hyperlink). The caller
//! checks the link first (`temor_core::terminal::link`); this only hands it over.

use temor_core::terminal::link::LinkTarget;
use thiserror::Error;

/// Why a link could not be opened.
#[derive(Debug, Error)]
pub enum OpenError {
    /// This system has no opener yet.
    #[error("links cannot be opened on this system yet")]
    Unsupported,
    /// The desktop's opener could not be started.
    #[error("the link could not be opened: {0}")]
    Failed(#[source] std::io::Error),
}

/// Hands a checked link to the desktop. Implemented by the platform's own handler.
pub trait Opener {
    fn open(&self, target: &LinkTarget) -> Result<(), OpenError>;
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Child, Command, Stdio};

    use temor_core::terminal::link::LinkTarget;

    use super::{OpenError, Opener};

    /// `xdg-open`, the freedesktop launcher every Linux desktop ships.
    #[derive(Debug, Default)]
    pub struct XdgOpen;

    impl Opener for XdgOpen {
        fn open(&self, target: &LinkTarget) -> Result<(), OpenError> {
            let child = Command::new("xdg-open")
                .arg(target.as_str())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(OpenError::Failed)?;
            reap_later(child);
            Ok(())
        }
    }

    /// Waits for the opener in the background so it never stays a zombie. The helper is the
    /// app edge's, so it may run a thread; the wait ends when the opener exits.
    fn reap_later(mut child: Child) {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

#[cfg(not(target_os = "linux"))]
mod other {
    use temor_core::terminal::link::LinkTarget;

    use super::{OpenError, Opener};

    /// Stub until the platform's opener is written.
    // TODO(platform:macos): NSWorkspace open; TODO(platform:windows): ShellExecute.
    #[derive(Debug, Default)]
    pub struct Unsupported;

    impl Opener for Unsupported {
        fn open(&self, _target: &LinkTarget) -> Result<(), OpenError> {
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
