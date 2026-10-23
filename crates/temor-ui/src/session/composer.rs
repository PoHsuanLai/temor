//! The composer (PLAN: block list): where the next command is typed while the shell waits at its
//! prompt, in place of the prompt's own rows. The text is the pane's, kept here with the
//! selection, so a drop, a paste or Re-run puts text in it whoever has the keyboard; Enter sends
//! it to the shell as a paste and a Return, and the arrows step through the pane's commands.

use temor_core::vocab::ids::BlockId;
use temor_pane::paste;

use super::Session;

/// Which way the arrows step through the commands run before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryStep {
    Older,
    Newer,
}

/// The composer's text and where it is in the history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composer {
    /// Whether the list shows it: the shell is at its prompt and nothing runs.
    shown: bool,
    text: String,
    /// The command shown from the history, counted from the newest (`Some(0)`), and what was
    /// typed before the first step back.
    step: Option<(usize, String)>,
}

/// The commands to step through, newest first: each run once, the most recent run kept.
fn newest_first<'a>(commands: impl DoubleEndedIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for command in commands.rev() {
        let command = command.trim_end();
        if !command.is_empty() && !out.contains(&command) {
            out.push(command);
        }
    }
    out
}

/// Where a step from `at` goes in `count` commands: one older or newer, `None` past the newest
/// (back to what was typed), and staying at the oldest.
fn stepped(at: Option<usize>, step: HistoryStep, count: usize) -> Option<usize> {
    match (step, at) {
        (_, _) if count == 0 => None,
        (HistoryStep::Older, None) => Some(0),
        (HistoryStep::Older, Some(n)) => Some((n + 1).min(count - 1)),
        (HistoryStep::Newer, None | Some(0)) => None,
        (HistoryStep::Newer, Some(n)) => Some(n - 1),
    }
}

impl Session {
    /// The list shows the composer, or stopped showing it.
    pub fn set_composer_shown(&mut self, shown: bool) {
        self.composer.shown = shown;
    }

    /// Whether the list shows the composer: typing goes there, not to the shell.
    pub fn composer_shown(&self) -> bool {
        self.composer.shown
    }

    /// Whether text is selected in the pane.
    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    /// The composer's text.
    pub fn composer_text(&self) -> &str {
        &self.composer.text
    }

    /// The person edited the composer's text: a step through the history starts again from it.
    pub fn set_composer_text(&mut self, text: String) {
        self.composer.text = text;
        self.composer.step = None;
    }

    /// Adds `text` at the end of the composer's text.
    pub fn composer_append(&mut self, text: &str) {
        let mut all = std::mem::take(&mut self.composer.text);
        all.push_str(text);
        self.set_composer_text(all);
    }

    /// Takes the last character off the composer's text.
    pub fn composer_backspace(&mut self) {
        let mut all = std::mem::take(&mut self.composer.text);
        all.pop();
        self.set_composer_text(all);
    }

    /// Shows the command one older or newer than the one shown; past the newest, what was typed.
    pub fn composer_history(&mut self, step: HistoryStep) {
        let ids: Vec<BlockId> = self.block_outcomes().into_iter().map(|b| b.0).collect();
        let commands = newest_first(ids.iter().filter_map(|&id| self.block_command(id)));
        let at = self.composer.step.as_ref().map(|(n, _)| *n);
        let to = stepped(at, step, commands.len());
        let shown = to.map(|n| commands[n].to_owned());
        let typed = match self.composer.step.take() {
            Some((_, typed)) => typed,
            None => self.composer.text.clone(),
        };
        self.composer.text = shown.unwrap_or_else(|| typed.clone());
        self.composer.step = to.map(|n| (n, typed));
    }

    /// Sends the composer's text to the shell, framed as a paste where the shell asked for that
    /// so no line break in it runs anything early, then a Return; the composer empties.
    pub fn composer_submit(&mut self) {
        let text = std::mem::take(&mut self.composer.text);
        self.composer.step = None;
        if !text.is_empty() {
            let mode = self.pane.with_term(|term| term.paste_mode());
            self.pane.send(&paste::encode(&text, mode));
        }
        self.pane.send(b"\r");
        self.wake_cursor();
    }

    /// Text the window types for the person (a drop, a paste): into the composer while it is
    /// shown, else to the shell as a paste. Whether the composer took it.
    pub(crate) fn composer_takes(&mut self, text: &str) -> bool {
        if self.composer.shown {
            self.composer_append(text);
        }
        self.composer.shown
    }
}

#[cfg(test)]
#[path = "composer_tests.rs"]
mod tests;
