//! The session bus an environment names, as a connection builder.

use crate::{BusRoute, DesktopEnv, DesktopError};

/// A builder for the session bus, or [`DesktopError::NoBus`] when the route is absent.
pub(super) fn session_builder(
    env: &DesktopEnv,
) -> Result<zbus::connection::Builder<'static>, DesktopError> {
    match &env.session {
        BusRoute::Usual => zbus::connection::Builder::session(),
        BusRoute::Address(address) => zbus::connection::Builder::address(address.as_str()),
        BusRoute::Absent => return Err(DesktopError::NoBus),
    }
    .map_err(|error| DesktopError::bus("connect to the session bus", error))
}

/// A connection to the session bus.
pub(super) async fn connect(env: &DesktopEnv) -> Result<zbus::Connection, DesktopError> {
    session_builder(env)?
        .build()
        .await
        .map_err(|error| DesktopError::bus("connect to the session bus", error))
}
