//! "New tabs start in" (`shell.start_in`): which folder a new tab and a split open in. Today's
//! way is the default: a tab in the home folder (or the Space's), a split in the folder of the
//! pane it came from. The two other choices are the folder of the pane in front, or home, for
//! both. A tab on a host and a tab given a folder are never changed.

use temor_core::workspace::{Spawn, WorkspaceIn};
use temor_pane::settings::StartIn;

use super::model::Hub;

impl Hub {
    /// A new tab with no folder named starts where the pane in front is, when the setting says so.
    /// Runs after the host and Space rules, so it only fills in what they left open.
    pub(super) fn start_prepared(&self, input: WorkspaceIn) -> WorkspaceIn {
        let plain = matches!(input, WorkspaceIn::NewTab { cwd: None });
        if !plain || self.launch.settings.start_in != StartIn::Current || self.ssh.intent.is_some()
        {
            return input;
        }
        let front = self
            .book
            .ws
            .front()
            .and_then(|tab| self.book.ws.focus_of(tab));
        match front.and_then(|pane| self.book.ws.cwds.get(&pane)) {
            Some(cwd) => WorkspaceIn::NewTab {
                cwd: Some(cwd.clone()),
            },
            None => input,
        }
    }

    /// Whether the pane that `input` starts belongs in the home folder, not where it came from.
    pub(super) fn split_starts_at_home(&self, input: &WorkspaceIn) -> bool {
        self.launch.settings.start_in == StartIn::Home
            && matches!(
                input,
                WorkspaceIn::Split { .. } | WorkspaceIn::SplitIn { .. }
            )
    }

    /// `spawn` without the folder it inherited, so its shell starts where a new shell does.
    pub(super) fn starting_at_home(&mut self, mut spawn: Spawn) -> Spawn {
        self.book.ws.cwds.remove(&spawn.pane);
        spawn.cwd = None;
        spawn
    }
}
