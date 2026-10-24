//! The composer (PLAN: block list): where the next command is typed while the shell waits at its
//! prompt, in place of the prompt's own rows. The text is the pane's, kept here with the
//! selection, so a drop, a paste or Re-run puts text in it whoever has the keyboard; Enter sends
//! it to the shell as a paste and a Return, and the arrows step through the pane's commands.
//!
//! Tab asks the shell to complete the line (`temor_shell::complete`): one word is put in, and
//! several are offered to pick from, with what they all start with put in already.

use temor_core::vocab::ids::BlockId;
use temor_pane::paste::{self, PasteMode};
use temor_shell::complete::{Answer, Completions, REQUEST_KEY, decode};

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
    /// What was last sent, until the command it starts takes it: a shell that does not say what
    /// it runs (nushell) shows a command of several lines on rows the screen cannot give back.
    sent: Option<String>,
    /// Whether the shell answers Tab: its script said so at this prompt.
    completes: bool,
    /// The line asked about, until the shell answers.
    asked: Option<String>,
    /// The words offered for the line's end.
    choices: Option<Choices>,
}

/// The words a shell offered for the line's end, one of them picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choices {
    completions: Completions,
    picked: usize,
}

impl Choices {
    /// The words, in the shell's order.
    pub fn words(&self) -> &[String] {
        &self.completions.words
    }

    /// The one picked.
    pub fn picked(&self) -> usize {
        self.picked
    }
}

/// `line` completed with `word`: a word that names a folder stays open for what is in it, and
/// any other is followed by a space for what comes next.
fn with_word(completions: &Completions, line: &str, word: &str) -> Option<String> {
    let mut out = completions.applied(line, word)?;
    if !word.ends_with('/') {
        out.push(' ');
    }
    Some(out)
}

/// What the shell's words make of `line`: the line with one word put in, or with what several
/// share put in and the words to pick from.
fn completed(line: &str, completions: Completions) -> (String, Option<Choices>) {
    match completions.words.as_slice() {
        [] => (line.to_owned(), None),
        [word] => (
            with_word(&completions, line, word).unwrap_or_else(|| line.to_owned()),
            None,
        ),
        _ => {
            let shared = completions.applied(line, completions.shared());
            // What they share never takes back what was typed.
            let text = shared
                .filter(|text| text.chars().count() > line.chars().count())
                .unwrap_or_else(|| line.to_owned());
            let choices = Choices {
                completions,
                picked: 0,
            };
            (text, Some(choices))
        }
    }
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

    /// The person edited the composer's text: a step through the history starts again from it,
    /// and the words offered for the old text are put away.
    pub fn set_composer_text(&mut self, text: String) {
        self.composer.text = text;
        self.composer.step = None;
        self.composer.asked = None;
        self.composer.choices = None;
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
        self.composer.asked = None;
        self.composer.choices = None;
    }

    /// Tab: asks the shell to complete the line. The line goes into the shell's own line as a
    /// paste (none of it runs), then the key its script answers; the script empties its line again.
    /// Nothing is asked of a shell that has not said it answers, nor while an answer is awaited.
    pub fn composer_complete(&mut self) {
        let composer = &self.composer;
        if !composer.shown || !composer.completes || composer.asked.is_some() {
            return;
        }
        let text = composer.text.clone();
        let mode = self.pane.with_term(|term| term.paste_mode());
        // A line break that is not framed as a paste would run the line.
        if text.contains('\n') && mode != PasteMode::Bracketed {
            return;
        }
        if !text.is_empty() {
            self.pane.send(&paste::encode(&text, mode));
        }
        self.pane.send(REQUEST_KEY);
        self.composer.asked = Some(text);
    }

    /// The shell's answer (OSC 7504): that it completes, or the words for the line asked about.
    /// An answer for a line since changed is dropped.
    pub(in crate::session) fn completion_answer(&mut self, payload: &str) {
        match decode(payload) {
            Ok(Answer::Ready) => self.composer.completes = true,
            Ok(Answer::Completed(completions)) => {
                if self.composer.asked.take().as_deref() != Some(self.composer.text.as_str()) {
                    return;
                }
                let (text, choices) = completed(&self.composer.text, completions);
                self.composer.text = text;
                self.composer.step = None;
                self.composer.choices = choices;
            }
            _ => {}
        }
    }

    /// A command started: the shell running it may be another, which answers no Tab until its
    /// prompt says so.
    pub(in crate::session) fn completion_ended(&mut self) {
        self.composer.completes = false;
        self.composer.asked = None;
        self.composer.choices = None;
    }

    /// Whether the shell answers Tab: its script said so at this prompt.
    pub fn composer_completes(&self) -> bool {
        self.composer.completes
    }

    /// The words offered for the line's end, if any.
    pub fn composer_choices(&self) -> Option<&Choices> {
        self.composer.choices.as_ref()
    }

    /// Picks the word above (`Older`) or below (`Newer`) the one picked, round the ends.
    pub fn composer_pick(&mut self, step: HistoryStep) {
        let Some(choices) = self.composer.choices.as_mut() else {
            return;
        };
        let count = choices.words().len().max(1);
        choices.picked = match step {
            HistoryStep::Older => (choices.picked + count - 1) % count,
            HistoryStep::Newer => (choices.picked + 1) % count,
        };
    }

    /// Puts the picked word in, or the one at `index` when given.
    pub fn composer_take_choice(&mut self, index: Option<usize>) {
        let Some(choices) = self.composer.choices.take() else {
            return;
        };
        let word = choices.words().get(index.unwrap_or(choices.picked));
        if let Some(text) =
            word.and_then(|w| with_word(&choices.completions, &self.composer.text, w))
        {
            self.composer.text = text;
        }
    }

    /// Puts the offered words away, the line as it is.
    pub fn composer_close_choices(&mut self) {
        self.composer.choices = None;
    }

    /// Sends the composer's text to the shell, framed as a paste where the shell asked for that
    /// so no line break in it runs anything early, then a Return; the composer empties.
    pub fn composer_submit(&mut self) {
        let text = std::mem::take(&mut self.composer.text);
        self.composer.step = None;
        self.completion_ended();
        if !text.is_empty() {
            let mode = self.pane.with_term(|term| term.paste_mode());
            self.pane.send(&paste::encode(&text, mode));
        }
        self.pane.send(b"\r");
        self.composer.sent = (!text.is_empty()).then_some(text);
        self.wake_cursor();
    }

    /// What the composer last sent, taken by the command it starts.
    pub(in crate::session) fn take_composer_sent(&mut self) -> Option<String> {
        self.composer.sent.take()
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
