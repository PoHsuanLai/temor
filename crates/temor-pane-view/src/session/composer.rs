//! The composer (PLAN: block list): where the next command is typed while the shell waits at its
//! prompt, in place of the prompt's own rows. The text is the pane's, kept here with the
//! selection, so a drop, a paste or Re-run puts text in it whoever has the keyboard; Enter sends
//! it to the shell as a paste and a Return, and the arrows step through the pane's commands.
//!
//! Tab asks the sources that complete the line (`temor_panes::shell::suggest`): the running shell's own
//! completion through the pane (`temor_panes::shell::complete`), and any source that stands on its own
//! (nushell's). What they say is merged into one list: one suggestion is put in, and several are
//! offered to pick from, with what they all start with put in already. A slow source adds its
//! suggestions to the list when they arrive.

use temor_client::model::vocab::ids::BlockId;
use temor_panes::pane::paste::{self, PasteMode};
use temor_panes::shell::complete::{Answer, REQUEST_KEY, decode};
use temor_panes::shell::suggest::{Answered, Ask, SourceId, Suggestion, common_start};

use super::{HistoryStep, Session};
use temor_panes::sources::SHELL;

/// The composer's text and where it is in the history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composer {
    /// Whether the list shows it: the shell is at its prompt and nothing runs.
    shown: bool,
    text: String,
    /// The command shown from the history, counted from the newest (`Some(0)`), and what was
    /// typed before the first step back.
    step: Option<(usize, String)>,
    /// Whether the shell answers Tab: its script said so at this prompt.
    shell_ready: bool,
    /// The requests made so far, numbered so that a late answer to an old one is dropped.
    requests: u64,
    /// The request the sources are answering.
    asked: Option<Asked>,
    /// The suggestions offered for the line.
    choices: Option<Choices>,
}

/// A request to the sources, and what they have said so far.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    request: u64,
    /// The line asked about.
    line: String,
    /// The sources that have not answered yet.
    waiting: Vec<SourceId>,
    merged: Vec<Suggestion>,
    /// Whether the line has been completed (or the list shown) from an answer already.
    presented: bool,
}

/// The suggestions offered for a line, one of them picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choices {
    /// The line they were asked for: each takes its place in it.
    base: String,
    suggestions: Vec<Suggestion>,
    picked: usize,
}

impl Choices {
    /// The words, in the order the sources gave them.
    pub fn words(&self) -> Vec<&str> {
        self.suggestions.iter().map(|s| s.word.as_str()).collect()
    }

    /// What each says about itself, in the same order.
    pub fn details(&self) -> Vec<Option<&str>> {
        self.suggestions
            .iter()
            .map(|s| s.detail.as_deref())
            .collect()
    }

    /// The one picked.
    pub fn picked(&self) -> usize {
        self.picked
    }
}

/// What the suggestions make of `line`: the line with one suggestion put in, or with what several
/// share put in and the suggestions to pick from.
fn completed(line: &str, suggestions: Vec<Suggestion>) -> (String, Option<Choices>) {
    match suggestions.as_slice() {
        [] => (line.to_owned(), None),
        [one] => (one.applied(line).unwrap_or_else(|| line.to_owned()), None),
        [first, ..] => {
            // What they share is put in only where they all take the same place.
            let same_place = suggestions
                .iter()
                .all(|s| (s.from, s.to) == (first.from, first.to));
            let words: Vec<&str> = suggestions.iter().map(|s| s.word.as_str()).collect();
            let shared = Suggestion {
                word: common_start(&words).to_owned(),
                from: first.from,
                to: first.to,
                space: false,
                detail: None,
            };
            let typed = first.to.saturating_sub(first.from);
            // What they share never takes back what was typed.
            let text = shared
                .applied(line)
                .filter(|_| same_place && shared.word.chars().count() > typed)
                .unwrap_or_else(|| line.to_owned());
            let choices = Choices {
                base: line.to_owned(),
                suggestions,
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

impl Composer {
    /// A source's answer: its suggestions join the others'. The first to say anything completes the
    /// line, or lists what to pick from; the ones after it add to the list. An answer to an old
    /// request, or from a source not asked, is dropped.
    fn merge(&mut self, answered: Answered) {
        let composer = self;
        let Some(asked) = composer.asked.as_mut() else {
            return;
        };
        if asked.request != answered.request || !asked.waiting.contains(&answered.source) {
            return;
        }
        asked.waiting.retain(|source| *source != answered.source);
        for suggestion in answered.suggestions {
            if !asked.merged.contains(&suggestion) {
                asked.merged.push(suggestion);
            }
        }
        match (
            &mut composer.choices,
            asked.merged.is_empty(),
            asked.presented,
        ) {
            // Offered already: the list grows, the pick stays.
            (Some(choices), false, _) => choices.suggestions = asked.merged.clone(),
            // Completed from one suggestion already: what comes after changes nothing.
            (None, _, true) | (_, true, _) => {}
            (None, false, false) => {
                asked.presented = true;
                let (text, choices) = completed(&asked.line, asked.merged.clone());
                composer.text = text;
                composer.step = None;
                composer.choices = choices;
            }
        }
        if asked.waiting.is_empty() || (composer.choices.is_none() && asked.presented) {
            composer.asked = None;
        }
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
        let known: Vec<String> = ids
            .iter()
            .filter_map(|&id| self.block_command(id))
            .collect();
        let commands = newest_first(known.iter().map(String::as_str));
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

    /// Tab: asks the sources to complete the line. The shell's goes into its own line as a paste
    /// (none of it runs), then the key its script answers; the script empties its line again.
    /// A source that is not ready is not asked, nor is anything asked while answers are awaited.
    pub fn composer_complete(&mut self) {
        let composer = &self.composer;
        if !composer.shown || composer.asked.is_some() {
            return;
        }
        let text = composer.text.clone();
        let mode = self.pane.with_term(|term| term.paste_mode());
        // A line break that is not framed as a paste would run the line.
        let shell = composer.shell_ready && (!text.contains('\n') || mode == PasteMode::Bracketed);
        let others = self.core().sources.any_ready();
        if !shell && !others {
            return;
        }
        self.composer.requests += 1;
        let request = self.composer.requests;
        let mut waiting = Vec::new();
        if shell {
            if !text.is_empty() {
                self.pane.send(&paste::encode(&text, mode));
            }
            self.pane.send(REQUEST_KEY);
            waiting.push(SHELL);
        }
        let core = self.core();
        if let Some(deliver) = core.sources.deliver.clone() {
            let ask = Ask {
                line: text.clone(),
                folder: core.folder.clone(),
            };
            for source in core.sources.list.iter().filter(|source| source.ready()) {
                waiting.push(source.id());
                source.ask(ask.clone(), request, deliver.clone());
            }
        }
        drop(core);
        self.composer.asked = Some(Asked {
            request,
            line: text,
            waiting,
            merged: Vec::new(),
            presented: false,
        });
    }

    /// The shell's answer (OSC 7504): that it completes, or the words for the line asked about.
    pub(in crate::session) fn completion_answer(&mut self, payload: &str) {
        match decode(payload) {
            Ok(Answer::Ready) => self.composer.shell_ready = true,
            Ok(Answer::Completed(completions)) => {
                let Some(asked) = self.composer.asked.as_ref() else {
                    return;
                };
                if !asked.waiting.contains(&SHELL) {
                    return;
                }
                self.completion_answered(Answered {
                    request: asked.request,
                    source: SHELL,
                    suggestions: completions.suggestions(&asked.line),
                });
            }
            _ => {}
        }
    }

    /// A source's answer (see [`Composer::merge`]).
    pub(in crate::session) fn completion_answered(&mut self, answered: Answered) {
        self.composer.merge(answered);
    }

    /// A command started: the shell running it may be another, which answers no Tab until its
    /// prompt says so.
    pub(in crate::session) fn completion_ended(&mut self) {
        self.composer.shell_ready = false;
        self.composer.asked = None;
        self.composer.choices = None;
    }

    /// Whether something answers Tab: the shell's script said so at this prompt, or a source of
    /// its own is ready.
    pub fn composer_completes(&self) -> bool {
        self.composer.shell_ready || self.core().sources.any_ready()
    }

    /// The suggestions offered for the line, if any.
    pub fn composer_choices(&self) -> Option<&Choices> {
        self.composer.choices.as_ref()
    }

    /// Picks the word above (`Older`) or below (`Newer`) the one picked, round the ends.
    pub fn composer_pick(&mut self, step: HistoryStep) {
        let Some(choices) = self.composer.choices.as_mut() else {
            return;
        };
        let count = choices.suggestions.len().max(1);
        choices.picked = match step {
            HistoryStep::Older => (choices.picked + count - 1) % count,
            HistoryStep::Newer => (choices.picked + 1) % count,
        };
    }

    /// Puts the picked suggestion in, or the one at `index` when given.
    pub fn composer_take_choice(&mut self, index: Option<usize>) {
        let Some(choices) = self.composer.choices.take() else {
            return;
        };
        let at = index.unwrap_or(choices.picked);
        if let Some(text) = choices
            .suggestions
            .get(at)
            .and_then(|suggestion| suggestion.applied(&choices.base))
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
        self.core().sent = (!text.is_empty()).then_some(text);
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
