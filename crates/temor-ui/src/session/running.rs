//! What the tab's row reads of a pane: the command that is running now and for how long.

use temor_core::block::Outcome;

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
        let block = self
            .blocks
            .store()
            .iter()
            .rev()
            .find(|block| block.outcome() == Outcome::Running)?;
        Some(Running {
            command: block.command.0.clone(),
            for_ms: self.clock.now().since(block.started),
        })
    }

    /// How the newest command stands; none before the first, or where the shell marks none.
    pub fn last_outcome(&self) -> Option<Outcome> {
        self.blocks
            .store()
            .iter()
            .next_back()
            .map(|block| block.outcome())
    }
}
