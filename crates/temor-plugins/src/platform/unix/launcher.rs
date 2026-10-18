//! Starting a plugin for one request.

use bayonet::run::{RunError, Runner, Session, Timeouts};

use super::wire::Wire;
use crate::capability::Cap;
use crate::catalog::{APP, Plugin};
use crate::error::PluginError;
use crate::message::{Message, Request};
use crate::waits::Waits;

/// Runs requests against plugins. Cloning is cheap and shares the log; every request starts the
/// plugin afresh and kills it (and what it started) on every path out.
#[derive(Debug, Clone)]
pub struct Launcher {
    pub(super) runner: Runner,
}

impl Launcher {
    /// A launcher that waits as `waits` say. A plugin's stderr lines go to `log`, already
    /// prefixed with the app and the plugin's id.
    pub fn new(waits: Waits, log: impl Fn(&str) + Send + Sync + 'static) -> Launcher {
        let timeouts = Timeouts::default()
            .with_hello(waits.hello)
            .with_silence(waits.silence)
            .with_cancel_grace(waits.cancel_grace);
        Launcher {
            runner: Runner::new(APP, timeouts).with_log(log),
        }
    }

    /// Starts `plugin`, checks it answers `capability`, and sends `request`. No reply carries more
    /// than `payload_limit` bytes of payload.
    pub(super) fn open(
        &self,
        plugin: &Plugin,
        capability: Cap,
        request: &Request,
        payload_limit: u64,
    ) -> Result<Session<Wire>, PluginError> {
        Ok(self
            .runner
            .open::<Wire>(plugin.installed(), capability, request, payload_limit)?)
    }
}

impl From<RunError<Cap>> for PluginError {
    fn from(error: RunError<Cap>) -> PluginError {
        let plugin = match &error {
            RunError::Silent { plugin, .. }
            | RunError::Crashed { plugin, .. }
            | RunError::Protocol { plugin, .. }
            | RunError::Version { plugin, .. }
            | RunError::Lacks { plugin, .. }
            | RunError::Cancelled { plugin } => plugin.to_string(),
            RunError::Spawn { program, .. } => program.clone(),
            _ => String::new(),
        };
        match error {
            RunError::Cancelled { .. } => PluginError::Cancelled { plugin },
            other => PluginError::Failed {
                plugin,
                reason: other.to_string(),
            },
        }
    }
}

/// The error for an answer that does not answer the request.
pub(super) fn unexpected(plugin: &Plugin, message: &Message) -> PluginError {
    PluginError::Broke {
        plugin: plugin.id().to_owned(),
        reason: format!("did not expect {message:?}"),
    }
}

/// The error for a plugin that said the request failed.
pub(super) fn said(plugin: &Plugin, message: String) -> PluginError {
    PluginError::Said {
        plugin: plugin.id().to_owned(),
        message,
    }
}
