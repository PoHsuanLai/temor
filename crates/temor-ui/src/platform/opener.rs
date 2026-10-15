//! Opening a link in the desktop's own handler (PLAN §6: ⌘-click on a hyperlink). The caller
//! checks the link first (`temor_pane::link`); this only hands it over.

use temor_pane::text::LinkTarget;
use thiserror::Error;

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

#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Child, Command, Stdio};

    use temor_desktop::{BusRoute, DesktopEnv, SystemLinks};
    use temor_pane::text::LinkTarget;

    use super::{OpenError, Opener};

    /// The desktop portal's `OpenURI` first, the way a sandboxed app must and any desktop
    /// supports; `xdg-open`, the freedesktop launcher, when no portal takes the link.
    #[derive(Debug, Default)]
    pub struct DesktopOpen {
        pub(super) env: DesktopEnv,
    }

    impl Opener for DesktopOpen {
        fn open(&self, target: &LinkTarget) -> Result<(), OpenError> {
            if self.env.session == BusRoute::Absent {
                return xdg_open(target.as_str()).map(reap_later);
            }
            // The portal is a bus call: ask it off the window's thread, and fall back there.
            let env = self.env.clone();
            let uri = target.as_str().to_owned();
            std::thread::spawn(move || {
                if !through_portal(&env, &uri)
                    && let Ok(mut child) = xdg_open(&uri)
                {
                    let _ = child.wait();
                }
            });
            Ok(())
        }
    }

    /// Whether the portal took `uri`. The call runs on a runtime of its own, on this thread.
    fn through_portal(env: &DesktopEnv, uri: &str) -> bool {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return false;
        };
        runtime.block_on(async {
            match SystemLinks::connect(env).await {
                Ok(links) => links.open(uri).await.is_ok(),
                Err(_) => false,
            }
        })
    }

    fn xdg_open(uri: &str) -> Result<Child, OpenError> {
        Command::new("xdg-open")
            .arg(uri)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(OpenError::Failed)
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
    use temor_pane::text::LinkTarget;

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
        Box::new(linux::DesktopOpen {
            env: temor_desktop::DesktopEnv::from_process(),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(other::Unsupported)
    }
}
