//! Opening a link in the desktop's own handler (PLAN §6: ⌘-click on a hyperlink). The caller
//! checks the link first (`temor_panes::pane::link`); this only hands it over.

use temor_client::desktop::{DesktopEnv, SystemOpen};
use temor_panes::pane::text::{LinkTarget, LinkVerdict, check_link};
use thiserror::Error;

use crate::words::notices;

/// Why a link could not be opened.
#[derive(Debug, Error)]
#[non_exhaustive]
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

/// Checks `target` and hands it to `opener`: the one way a link opens, from a click in a pane, a
/// plugin or the viewer. The error is the sentence to tell the person (`crate::words::notices`).
pub fn open_link(opener: &dyn Opener, target: &str) -> Result<(), &'static str> {
    match check_link(target) {
        LinkVerdict::Open(link) => opener.open(&link).map_err(|_| notices::LINK_NOT_OPENED),
        LinkVerdict::Refused(_) => Err(notices::LINK_REFUSED),
    }
}

/// The desktop's opener (`temor_client::desktop::SystemOpen`), asked as `env` says.
#[derive(Debug, Default)]
pub struct DesktopOpen {
    env: DesktopEnv,
}

impl Opener for DesktopOpen {
    fn open(&self, target: &LinkTarget) -> Result<(), OpenError> {
        SystemOpen::open_link(&self.env, target.as_str()).map_err(OpenError::from_io)
    }
}

impl OpenError {
    /// The opener's failure: a system with none is [`OpenError::Unsupported`].
    pub fn from_io(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::Unsupported => OpenError::Unsupported,
            _ => OpenError::Failed(error),
        }
    }
}

/// The platform's opener.
pub fn default_opener() -> Box<dyn Opener> {
    Box::new(DesktopOpen {
        env: DesktopEnv::from_process(),
    })
}
