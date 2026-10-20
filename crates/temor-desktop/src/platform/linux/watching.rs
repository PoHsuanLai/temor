//! The watch of an agent over the session bus: the assistant's Terminals recognise the app and
//! its conversation from the command line, and `Checkpoint.WatchAgent` opens (or finds) the
//! session and says who closes it. Without a recognised app, or when docket does not know
//! `WatchAgent`, the watch is the plain `Checkpoint.Watch`.

use super::checkpoints::{SystemCheckpoints, client_fault, failed};
use crate::DesktopError;
use crate::checkpoints::{Named, Rewind, SessionKey, WatchAsk, WatchOwner, Watching};
use docket_core::{SeenCommand, WatchedAgent, Workspace};

impl SystemCheckpoints {
    /// The session that keeps restore points for the agent `ask` describes.
    pub(super) fn watch_agent(&self, ask: &WatchAsk) -> Result<Watching, DesktopError> {
        let workspace = Workspace::parse(&ask.workspace).map_err(|_| failed())?;
        let rewind = match ask.rewind {
            Rewind::Docket => docket_core::Rewind::Docket,
            Rewind::Agent => docket_core::Rewind::Agent,
        };
        let label = ask.label.clone();
        if let Some(agent) = self.recognise(ask) {
            let named = self.runtime.block_on(self.intents.checkpoint_watch_agent(
                workspace.clone(),
                label.clone(),
                rewind,
                agent,
            ));
            // An answer this build cannot get (docket older than the call) is not fatal: the
            // plain watch still keeps restore points.
            if let Ok(watching) = named {
                let owner = match watching.owner {
                    docket_core::WatchOwner::Terminal => WatchOwner::Terminal,
                    docket_core::WatchOwner::Companion => WatchOwner::Companion,
                };
                let session = SessionKey::new(watching.session.as_str()).ok_or_else(failed)?;
                return Ok(Watching {
                    session,
                    owner,
                    named: Named::App,
                });
            }
        }
        let id = self
            .runtime
            .block_on(self.intents.checkpoint_watch(workspace, label, rewind))
            .map_err(|error| client_fault("watch the agent", &error))?;
        SessionKey::new(id.as_str())
            .map(Watching::plain)
            .ok_or_else(failed)
    }

    /// The configured app and conversation the assistant recognises in `ask`'s command line, if
    /// it does. Any trouble (no command line, no assistant, a failed call, an answer this build
    /// cannot read) is "not recognised", and the watch is the plain one. The command line is
    /// already from the program word on, so nothing in front of it is sent.
    fn recognise(&self, ask: &WatchAsk) -> Option<WatchedAgent> {
        let terminals = self.terminals.as_ref()?;
        let seen = SeenCommand::parse(ask.seen.as_ref()?.tokens().to_vec()).ok()?;
        let body = serde_json::to_string(&seen).ok()?;
        let answer = self.runtime.block_on(terminals.recognise(&body)).ok()?;
        serde_json::from_str::<Option<WatchedAgent>>(&answer)
            .ok()
            .flatten()
    }
}
