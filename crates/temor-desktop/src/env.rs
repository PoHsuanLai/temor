//! Where the session bus is, chosen once by the binary. Nothing in this crate reads an
//! environment variable or a bus address itself.

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
}

impl DesktopEnv {
    /// The process's own session bus.
    pub fn from_process() -> Self {
        Self::default()
    }

    /// A private bus at `address`.
    pub fn at(address: impl Into<String>) -> Self {
        Self {
            session: BusRoute::Address(address.into()),
        }
    }

    /// No bus, for tests of the absent path and builds without a desktop.
    pub fn absent() -> Self {
        Self {
            session: BusRoute::Absent,
        }
    }
}
