//! The file's document and `temor-pane`'s typed settings, both ways. The way in checks the
//! values the schema cannot (a family or program name, a command for the shell offer) with the
//! same readers `terminal.conf` used, so a bad one costs only itself.

use chordkit::Overrides;
use temor_pane::SettingsError;
use temor_pane::settings::{
    FontChoice, ShellChoice, TerminalSettings, font_family, offer_command, shell,
};

use super::doc::{
    CursorSettings, FontSettings, NotifySettings, OfferSettings, ScrollbackSettings, ShellSettings,
    TerminalDoc,
};
use super::values::{FontSize, LineSpacing, NotifySeconds, ScrollbackSize};

impl TerminalDoc {
    /// The settings the pane reads, with the person's `keys` (from `keys.conf`) put in. What the
    /// document holds that cannot be used comes back as problems, in words for the person, and
    /// the setting falls back to its default.
    pub fn terminal(&self, keys: Overrides) -> (TerminalSettings, Vec<SettingsError>) {
        let mut problems = Vec::new();
        let font = keep(font_family("font.family", &self.font.family), &mut problems);
        let program = keep(shell("shell.program", &self.shell.program), &mut problems);
        let offer_add = commands("offer.add", &self.offer.add, &mut problems);
        let offer_remove = commands("offer.remove", &self.offer.remove, &mut problems);
        let settings = TerminalSettings {
            font,
            font_size: self.font.size.0,
            line_height: self.font.line_height.0,
            scrollback: self.scrollback.lines.0,
            cursor: self.cursor.style.into(),
            cursor_blink: self.cursor.blink.into(),
            shell: program,
            start_in: self.shell.start_in.into(),
            pager: self.shell.pager.into(),
            keep_running: self.shell.keep_running.into(),
            keys,
            notify_after: self.notify.after.0,
            offer_add,
            offer_remove,
        };
        (settings, problems)
    }
}

/// The document that holds `settings` (its keys are not part of it: they have their own file).
pub fn from_terminal(settings: &TerminalSettings) -> TerminalDoc {
    TerminalDoc {
        font: FontSettings {
            family: match &settings.font {
                FontChoice::SystemMonospace => "system".to_owned(),
                FontChoice::Named(name) => name.clone(),
            },
            size: FontSize(settings.font_size),
            line_height: LineSpacing(settings.line_height),
        },
        cursor: CursorSettings {
            style: settings.cursor.into(),
            blink: settings.cursor_blink.into(),
        },
        scrollback: ScrollbackSettings {
            lines: ScrollbackSize(settings.scrollback),
        },
        shell: ShellSettings {
            program: match &settings.shell {
                ShellChoice::Login => "login".to_owned(),
                ShellChoice::Program(program) => program.clone(),
            },
            start_in: settings.start_in.into(),
            pager: settings.pager.into(),
            keep_running: settings.keep_running.into(),
        },
        notify: NotifySettings {
            after: NotifySeconds(settings.notify_after),
        },
        offer: OfferSettings {
            add: settings.offer_add.clone(),
            remove: settings.offer_remove.clone(),
        },
    }
}

/// `result`'s value, or the setting's default with the problem noted.
fn keep<T: Default>(result: Result<T, SettingsError>, problems: &mut Vec<SettingsError>) -> T {
    result.unwrap_or_else(|problem| {
        problems.push(problem);
        T::default()
    })
}

/// The commands of a shell-offer list that read, in order; the others are noted.
fn commands(key: &str, given: &[String], problems: &mut Vec<SettingsError>) -> Vec<String> {
    given
        .iter()
        .filter_map(|command| match offer_command(key, command) {
            Ok(command) => Some(command),
            Err(problem) => {
                problems.push(problem);
                None
            }
        })
        .collect()
}
