//! The rules: which block or program event becomes a notice, and how often notices may come.

use ds_core::time::stamp::Stamp;

use super::model::{Focus, Notice};
use crate::block::{Block, Outcome};
use temor_pane::settings::NotifyAfter;

/// The notice for a finished block, when it is worth one: the pane is unfocused, the setting is
/// on, and the command ran at least the threshold. `None` for a block that has not finished.
pub fn finished_notice(block: &Block, after: NotifyAfter, focus: Focus) -> Option<Notice> {
    let threshold = after.threshold_ms()?;
    let took_ms = block.elapsed_ms()?;
    let outcome = block.outcome();
    (focus == Focus::Unfocused && took_ms >= threshold && outcome != Outcome::Running).then(|| {
        Notice::CommandFinished {
            outcome,
            took_ms,
            command: block.command.0.clone(),
        }
    })
}

/// The notice for a program's notification, when the pane is unfocused.
pub fn program_notice(title: &str, body: &str, focus: Focus) -> Option<Notice> {
    (focus == Focus::Unfocused).then(|| Notice::Program {
        title: title.to_owned(),
        body: body.to_owned(),
    })
}

/// The notice for a bell, when the pane is unfocused.
pub fn bell_notice(focus: Focus) -> Option<Notice> {
    (focus == Focus::Unfocused).then_some(Notice::Bell)
}

/// Lets through at most one notice per `gap_ms`, so a program that notifies in a loop cannot
/// flood the desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limiter {
    gap_ms: u64,
    last: Option<Stamp>,
}

impl Limiter {
    /// The default gap between notices: two seconds.
    pub const DEFAULT_GAP_MS: u64 = 2000;

    pub fn new(gap_ms: u64) -> Self {
        Self { gap_ms, last: None }
    }

    /// Whether a notice may go out at `now`. A notice that goes out starts the gap.
    pub fn allow(&mut self, now: Stamp) -> bool {
        let open = self.last.is_none_or(|last| now.since(last) >= self.gap_ms);
        if open {
            self.last = Some(now);
        }
        open
    }
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new(Self::DEFAULT_GAP_MS)
    }
}
