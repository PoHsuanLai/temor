//! The dock badge: `com.canonical.Unity.LauncherEntry.Update`, a broadcast the desktop's dock
//! reads for a count, a progress bar and an urgent mark. docket and ds-desktop only read it
//! (sill's dock), so temor sends it itself.

use super::bus::connect;
use crate::{Attention, Badge, BadgeCount, DesktopEnv, DesktopError};
use std::collections::HashMap;
use zbus::zvariant::Value;

const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";
const PATH: &str = "/com/canonical/unity/launcherentry/1";
/// The desktop file's id, as the dock names the app.
const APP_URI: &str = "application://org.quire.Temor.desktop";

/// The dock badge on the session bus.
#[derive(Debug, Clone)]
pub struct SystemDock {
    connection: zbus::Connection,
}

impl SystemDock {
    /// A badge publisher on the session bus of `env`.
    pub async fn connect(env: &DesktopEnv) -> Result<Self, DesktopError> {
        Ok(Self {
            connection: connect(env).await?,
        })
    }

    /// Show `badge` on temor's icon. A default badge clears it.
    pub async fn show(&self, badge: &Badge) -> Result<(), DesktopError> {
        self.connection
            .emit_signal(
                None::<&str>,
                PATH,
                INTERFACE,
                "Update",
                &(APP_URI, launcher_update(badge)),
            )
            .await
            .map_err(|error| DesktopError::bus("update the dock badge", error))
    }
}

/// The `a{sv}` of an `Update` signal for `badge`. Every key is written, so a cleared badge
/// really clears what an earlier one set.
pub(super) fn launcher_update(badge: &Badge) -> HashMap<&'static str, Value<'static>> {
    let (count, count_visible) = match badge.count {
        Some(BadgeCount(count)) => (i64::from(count), true),
        None => (0, false),
    };
    let (progress, progress_visible) = match badge.progress {
        Some(progress) => (f64::from(progress.get()) / 100.0, true),
        None => (0.0, false),
    };
    let urgent = matches!(badge.attention, Attention::Needed);
    HashMap::from([
        ("count", Value::from(count)),
        ("count-visible", Value::from(count_visible)),
        ("progress", Value::from(progress)),
        ("progress-visible", Value::from(progress_visible)),
        ("urgent", Value::from(urgent)),
    ])
}
