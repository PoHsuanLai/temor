//! What the tab's row reads of a pane: the command that is running now and for how long.

use temor_client::model::block::Outcome;

use super::Session;

/// A command that has started and not yet ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    /// The command line as typed.
    pub command: String,
    /// Milliseconds since it started.
    pub for_ms: u64,
}

impl Session {
    /// The newest command that is still running, if the shell marks its commands.
    pub fn running_command(&self) -> Option<Running> {
        let core = self.core();
        let block = core
            .blocks
            .store()
            .iter()
            .rev()
            .find(|block| block.outcome() == Outcome::Running)?;
        Some(Running {
            command: block.command.0.clone(),
            for_ms: core.clock.now().since(block.started),
        })
    }

    /// Sends `bytes` to the program as if typed (an interrupt, for one).
    pub fn send_bytes(&self, bytes: &[u8]) {
        self.pane.send(bytes);
    }
}
