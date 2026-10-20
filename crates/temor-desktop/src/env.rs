//! Where the session bus is, chosen once by the binary. Nothing in this crate reads an
//! environment variable or a bus address itself.

use std::path::PathBuf;

/// How the session bus is reached.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BusRoute {
    /// The process's own: `DBUS_SESSION_BUS_ADDRESS`, else the standard socket.
    #[default]
    Usual,
    /// A bus by address: a private `dbus-daemon` for tests and dev scripts.
    Address(String),
    /// No bus at all; every service reports [`crate::DesktopError::NoBus`].
    Absent,
}

/// The seams the desktop services are built with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesktopEnv {
    /// The session bus.
    pub session: BusRoute,
    /// The folder a launch finds the running temor in, where there is no bus to ask (macOS): a
    /// private folder of the person's, made when it is first needed. None means every launch is
    /// the first.
    pub instance_dir: Option<PathBuf>,
}

impl DesktopEnv {
    /// The process's own session bus.
    pub fn from_process() -> Self {
        Self {
            // macOS gives each person their own temporary folder (`$TMPDIR`), which is as private
            // as a runtime directory there; elsewhere the bus finds the running temor.
            instance_dir: cfg!(target_os = "macos")
                .then(|| std::env::temp_dir().join("temor-instance")),
            ..Self::default()
        }
    }

    /// A private bus at `address`.
    pub fn at(address: impl Into<String>) -> Self {
        Self {
            session: BusRoute::Address(address.into()),
            ..Self::default()
        }
    }

    /// No bus, for tests of the absent path and builds without a desktop.
    pub fn absent() -> Self {
        Self {
            session: BusRoute::Absent,
            ..Self::default()
        }
    }

    /// The same, finding the running temor in `dir`.
    pub fn with_instance_dir(self, dir: impl Into<PathBuf>) -> Self {
        Self {
            instance_dir: Some(dir.into()),
            ..self
        }
    }
}
