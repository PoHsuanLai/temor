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

use temor_panes::pane::paste::{self, PasteMode};
use temor_panes::shell::complete::{Answer, REQUEST_KEY, decode};
use temor_panes::shell::suggest::{Answered, Ask, SourceId, Suggestion, common_start};

use super::{HistoryStep, Session};
use temor_panes::sources::SHELL;

/// The composer's text and where it is in the history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composer {
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

/// What happens to the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComposerIn {
    /// The person edited the text: a step through the history starts again from it, and the
    /// words offered for the old text are put away.
    Edit(String),
    /// Text added at the end.
    Append(String),
    /// The last character taken off.
    Backspace,
    /// A step through `commands` (the pane's, oldest first) one older or newer; past the newest,
    /// what was typed.
    History {
        step: HistoryStep,
        commands: Vec<String>,
    },
    /// Tab: ask the sources to complete the line, while the composer is `shown`. `mode` is how
    /// the shell takes a paste; `ready` the sources besides the shell that can answer.
    Complete {
        shown: bool,
        mode: PasteMode,
        ready: Vec<SourceId>,
    },
    /// The shell said it answers Tab, or what the line asked about can become.
    Shell(Answer),
    /// A source's answer.
    Answered(Answered),
    /// A command started: the shell running it may be another, which answers no Tab until its
    /// prompt says so.
    Started,
    /// The word above (`Older`) or below (`Newer`) the one picked, round the ends.
    Pick(HistoryStep),
    /// Put the picked word in, or the one at this index.
    Take(Option<usize>),
    /// Put the offered words away, the line as it is.
    CloseChoices,
    /// Enter: the text goes to the shell, framed as `mode` says.
    Submit { mode: PasteMode },
}

/// What the composer asks of its pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComposerOut {
    /// Bytes for the shell.
    Send(Vec<u8>),
    /// Ask these sources to complete `line`.
    Ask {
        request: u64,
        line: String,
        sources: Vec<SourceId>,
    },
    /// A command was sent (the text, if any): the pane notes it and wakes its cursor.
    Sent(Option<String>),
}

impl Composer {
    pub(crate) fn step(&mut self, input: ComposerIn) -> Vec<ComposerOut> {
        match input {
            ComposerIn::Edit(text) => self.edit(text),
            ComposerIn::Append(more) => {
                let text = format!("{}{more}", self.text);
                self.edit(text);
            }
            ComposerIn::Backspace => {
                let mut text = std::mem::take(&mut self.text);
                text.pop();
                self.edit(text);
            }
            ComposerIn::History { step, commands } => self.history(step, &commands),
            ComposerIn::Complete { shown, mode, ready } => {
                return self.complete(shown, mode, ready);
            }
            ComposerIn::Shell(Answer::Ready) => self.shell_ready = true,
            ComposerIn::Shell(Answer::Completed(completions)) => {
                if let Some(asked) = self.asked.as_ref() {
                    let answered = Answered {
                        request: asked.request,
                        source: SHELL,
                        suggestions: completions.suggestions(&asked.line),
                    };
                    self.merge(answered);
                }
            }
            ComposerIn::Shell(_) => {}
            ComposerIn::Answered(answered) => self.merge(answered),
            ComposerIn::Started => self.started(),
            ComposerIn::Pick(step) => self.pick(step),
            ComposerIn::Take(index) => self.take(index),
            ComposerIn::CloseChoices => self.choices = None,
            ComposerIn::Submit { mode } => return self.submit(mode),
        }
        Vec::new()
    }

    fn edit(&mut self, text: String) {
        self.text = text;
        self.step = None;
        self.asked = None;
        self.choices = None;
    }

    fn history(&mut self, step: HistoryStep, commands: &[String]) {
        let commands = newest_first(commands.iter().map(String::as_str));
        let at = self.step.as_ref().map(|(n, _)| *n);
        let to = stepped(at, step, commands.len());
        let shown = to.map(|n| commands[n].to_owned());
        let typed = match self.step.take() {
            Some((_, typed)) => typed,
            None => self.text.clone(),
        };
        self.text = shown.unwrap_or_else(|| typed.clone());
        self.step = to.map(|n| (n, typed));
        self.asked = None;
        self.choices = None;
    }

    /// The shell's request goes into its own line as a paste (none of it runs), then the key its
    /// script answers; the script empties its line again. A source that is not ready is not
    /// asked, nor is anything asked while answers are awaited.
    fn complete(&mut self, shown: bool, mode: PasteMode, ready: Vec<SourceId>) -> Vec<ComposerOut> {
        if !shown || self.asked.is_some() {
            return Vec::new();
        }
        // A line break that is not framed as a paste would run the line.
        let shell = self.shell_ready && (!self.text.contains('\n') || mode == PasteMode::Bracketed);
        if !shell && ready.is_empty() {
            return Vec::new();
        }
        self.requests += 1;
        let (request, line) = (self.requests, self.text.clone());
        let mut out = Vec::new();
        let mut waiting = Vec::new();
        if shell {
            if !line.is_empty() {
                out.push(ComposerOut::Send(paste::encode(&line, mode)));
            }
            out.push(ComposerOut::Send(REQUEST_KEY.to_vec()));
            waiting.push(SHELL);
        }
        if !ready.is_empty() {
            waiting.extend(ready.iter().copied());
            out.push(ComposerOut::Ask {
                request,
                line: line.clone(),
                sources: ready,
            });
        }
        self.asked = Some(Asked {
            request,
            line,
            waiting,
            merged: Vec::new(),
            presented: false,
        });
        out
    }

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

    fn started(&mut self) {
        self.shell_ready = false;
        self.asked = None;
        self.choices = None;
    }

    fn pick(&mut self, step: HistoryStep) {
        let Some(choices) = self.choices.as_mut() else {
            return;
        };
        let count = choices.suggestions.len().max(1);
        choices.picked = match step {
            HistoryStep::Older => (choices.picked + count - 1) % count,
            HistoryStep::Newer => (choices.picked + 1) % count,
        };
    }

    fn take(&mut self, index: Option<usize>) {
        let Some(choices) = self.choices.take() else {
            return;
        };
        let at = index.unwrap_or(choices.picked);
        if let Some(text) = choices
            .suggestions
            .get(at)
            .and_then(|suggestion| suggestion.applied(&choices.base))
        {
            self.text = text;
        }
    }

    /// The text goes to the shell, framed as a paste where the shell asked for that so no line
    /// break in it runs anything early, then a Return; the composer empties.
    fn submit(&mut self, mode: PasteMode) -> Vec<ComposerOut> {
        let text = std::mem::take(&mut self.text);
        self.step = None;
        self.started();
        let mut out = Vec::new();
        if !text.is_empty() {
            out.push(ComposerOut::Send(paste::encode(&text, mode)));
        }
        out.push(ComposerOut::Send(b"\r".to_vec()));
        out.push(ComposerOut::Sent((!text.is_empty()).then_some(text)));
        out
    }
}

impl Session {
    /// Whether the list shows the composer (the shell is at its prompt and nothing runs): typing
    /// goes there, not to the shell.
    pub fn composer_shown(&self) -> bool {
        self.list.as_ref().is_some_and(|list| list.composing)
    }

    /// Whether text is selected in the pane.
    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    /// The composer's text.
    pub fn composer_text(&self) -> &str {
        &self.composer.text
    }

    /// Steps the composer and carries out what it asks.
    pub(crate) fn composer_step(&mut self, input: ComposerIn) {
        for out in self.composer.step(input) {
            match out {
                ComposerOut::Send(bytes) => self.pane.send(&bytes),
                ComposerOut::Ask {
                    request,
                    line,
                    sources,
                } => {
                    let core = self.core();
                    let Some(deliver) = core.sources.deliver.clone() else {
                        continue;
                    };
                    let ask = Ask {
                        line,
                        folder: core.folder.clone(),
                    };
                    for source in core
                        .sources
                        .list
                        .iter()
                        .filter(|s| sources.contains(&s.id()))
                    {
                        source.ask(ask.clone(), request, deliver.clone());
                    }
                }
                ComposerOut::Sent(text) => {
                    self.core().sent = text;
                    self.wake_cursor();
                }
            }
        }
    }

    /// The person edited the composer's text (see [`ComposerIn::Edit`]).
    pub fn set_composer_text(&mut self, text: String) {
        self.composer_step(ComposerIn::Edit(text));
    }

    /// Adds `text` at the end of the composer's text.
    pub fn composer_append(&mut self, text: &str) {
        self.composer_step(ComposerIn::Append(text.to_owned()));
    }

    /// Takes the last character off the composer's text.
    pub fn composer_backspace(&mut self) {
        self.composer_step(ComposerIn::Backspace);
    }

    /// Shows the command one older or newer than the one shown; past the newest, what was typed.
    pub fn composer_history(&mut self, step: HistoryStep) {
        let commands = self
            .block_outcomes()
            .into_iter()
            .filter_map(|(id, ..)| self.block_command(id))
            .collect();
        self.composer_step(ComposerIn::History { step, commands });
    }

    /// Tab: asks the sources to complete the line (see [`ComposerIn::Complete`]).
    pub fn composer_complete(&mut self) {
        let shown = self.composer_shown();
        let mode = self.pane.with_term(|term| term.paste_mode());
        let ready = {
            let core = self.core();
            let sources = &core.sources;
            match sources.deliver {
                Some(_) => sources
                    .list
                    .iter()
                    .filter(|s| s.ready())
                    .map(|s| s.id())
                    .collect(),
                None => Vec::new(),
            }
        };
        self.composer_step(ComposerIn::Complete { shown, mode, ready });
    }

    /// The shell's answer (OSC 7504): that it completes, or the words for the line asked about.
    pub(in crate::session) fn completion_answer(&mut self, payload: &str) {
        if let Ok(answer) = decode(payload) {
            self.composer_step(ComposerIn::Shell(answer));
        }
    }

    /// A source's answer (see [`Composer::merge`]).
    pub(in crate::session) fn completion_answered(&mut self, answered: Answered) {
        self.composer_step(ComposerIn::Answered(answered));
    }

    /// A command started (see [`ComposerIn::Started`]).
    pub(in crate::session) fn completion_ended(&mut self) {
        self.composer_step(ComposerIn::Started);
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
        self.composer_step(ComposerIn::Pick(step));
    }

    /// Puts the picked suggestion in, or the one at `index` when given.
    pub fn composer_take_choice(&mut self, index: Option<usize>) {
        self.composer_step(ComposerIn::Take(index));
    }

    /// Puts the offered words away, the line as it is.
    pub fn composer_close_choices(&mut self) {
        self.composer_step(ComposerIn::CloseChoices);
    }

    /// Sends the composer's text to the shell (see [`ComposerIn::Submit`]).
    pub fn composer_submit(&mut self) {
        let mode = self.pane.with_term(|term| term.paste_mode());
        self.composer_step(ComposerIn::Submit { mode });
    }

    /// Text the window types for the person (a drop, a paste): into the composer while it is
    /// shown, else to the shell as a paste. Whether the composer took it.
    pub(crate) fn composer_takes(&mut self, text: &str) -> bool {
        let shown = self.composer_shown();
        if shown {
            self.composer_append(text);
        }
        shown
    }
}

#[cfg(test)]
#[path = "composer_tests.rs"]
mod tests;
