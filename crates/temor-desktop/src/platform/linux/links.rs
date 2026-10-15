//! Opening a link through the desktop portal: `org.freedesktop.portal.OpenURI`, which works the
//! same inside and outside a Flatpak sandbox and lets the desktop ask which program to use.

use super::bus::connect;
use crate::{DesktopEnv, DesktopError};
use std::collections::HashMap;
use zbus::zvariant::{OwnedObjectPath, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.OpenURI";

/// The desktop portal's link opener on the session bus.
#[derive(Debug, Clone)]
pub struct SystemLinks {
    connection: zbus::Connection,
}

impl SystemLinks {
    /// An opener on the session bus of `env`.
    pub async fn connect(env: &DesktopEnv) -> Result<Self, DesktopError> {
        Ok(Self {
            connection: connect(env).await?,
        })
    }

    /// Asks the portal to open `uri`. Done once the portal has taken the request; whether a
    /// program then opens it is the desktop's to show. An error means no portal took it.
    pub async fn open(&self, uri: &str) -> Result<(), DesktopError> {
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.connection
            .call_method(
                Some(DESTINATION),
                PATH,
                Some(INTERFACE),
                "OpenURI",
                &("", uri, options),
            )
            .await
            .and_then(|reply| reply.body().deserialize::<OwnedObjectPath>())
            .map(|_request| ())
            .map_err(|error| DesktopError::bus("open a link through the desktop portal", error))
    }
}
