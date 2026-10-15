//! What the shell offer (PLAN P17) reads from a session: which listed command is running, the
//! time on the session's clock, the screen kind and the last line. It reads; nothing here
//! changes the session.

use ds_core::time::stamp::Stamp;
use temor_core::block::BlockState;
use temor_core::offer::{Line, OfferList, Program, Screen, asks_for_secret};
use temor_core::vocab::ids::BlockId;

use super::Session;

impl Session {
    /// The time on the session's clock.
    pub fn now(&self) -> Stamp {
        self.clock.now()
    }

    /// The running command's line, as the shell marked it.
    pub fn running_command(&self) -> Option<&str> {
        match &self.blocks.machine {
            BlockState::Running(block) => Some(block.command.as_str()),
            _ => None,
        }
    }

    /// The running command's block, when its command is on `list`, and which entry matched.
    pub fn running_listed(&self, list: &OfferList) -> Option<(BlockId, Program)> {
        match &self.blocks.machine {
            BlockState::Running(block) => list
                .matches(block.command.as_str())
                .map(|program| (block.id, program)),
            _ => None,
        }
    }

    /// Whether a full-screen program has the screen.
    pub fn screen_kind(&self) -> Screen {
        match self.pane.with_term(|term| term.on_alternate_screen()) {
            true => Screen::Alternate,
            false => Screen::Normal,
        }
    }

    /// The last line with text on the screen (empty when the screen is blank).
    pub fn last_line(&self) -> String {
        let text = self.screen_text();
        let last = text.lines().rev().find(|line| !line.trim().is_empty());
        last.unwrap_or_default().to_owned()
    }

    /// The last line with text on the screen, as far as the offer cares.
    pub fn last_line_kind(&self) -> Line {
        match asks_for_secret(&self.last_line()) {
            true => Line::Asking,
            false => Line::Plain,
        }
    }
}
