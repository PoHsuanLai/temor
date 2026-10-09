//! The pane's settings file text: one `key = value` per line, `#` lines are comments. A bad
//! line is reported and skipped, so one typo never costs the rest of the file (PLAN §14).

use ds_core::word::Word;

use super::action::Action;
use super::chord::Chord;
use super::keymap::Keymap;
use super::settings_value::{
    CursorStyle, FontChoice, FontPoints, ScrollbackLines, ShellChoice, cursor, font_family,
    font_points, notify_after, scrollback, shell,
};
use crate::error::CoreError;
use crate::notify::NotifyAfter;

/// Everything the pane reads from its settings.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalSettings {
    pub font: FontChoice,
    pub font_size: FontPoints,
    pub scrollback: ScrollbackLines,
    pub cursor: CursorStyle,
    pub shell: ShellChoice,
    pub keymap: Keymap,
    /// When a finished command is worth a notice while the pane is unfocused (PLAN §8.1).
    pub notify_after: NotifyAfter,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font: FontChoice::default(),
            font_size: FontPoints::DEFAULT,
            scrollback: ScrollbackLines::DEFAULT,
            cursor: CursorStyle::default(),
            shell: ShellChoice::default(),
            keymap: Keymap::defaults(),
            notify_after: NotifyAfter::DEFAULT,
        }
    }
}

/// The settings and the lines that were refused, each with its 1-based line number.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSettings {
    pub settings: TerminalSettings,
    pub issues: Vec<(usize, CoreError)>,
}

/// Reads settings text over the defaults. Every problem is kept in `issues`; the rest applies.
pub fn parse_settings(text: &str) -> ParsedSettings {
    let mut settings = TerminalSettings::default();
    let mut issues = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Err(err) = apply_line(&mut settings, line) {
            issues.push((index + 1, err));
        }
    }
    ParsedSettings { settings, issues }
}

fn apply_line(settings: &mut TerminalSettings, line: &str) -> Result<(), CoreError> {
    let (key, raw) = line
        .split_once('=')
        .ok_or_else(|| CoreError::UnknownSetting(line.to_owned()))?;
    let key = key.trim();
    let value = unquote(raw.trim());
    match key {
        "font.family" => settings.font = font_family(key, value)?,
        "font.size" => settings.font_size = font_points(key, value)?,
        "scrollback.lines" => settings.scrollback = scrollback(key, value)?,
        "cursor.style" => settings.cursor = cursor(key, value)?,
        "shell.program" => settings.shell = shell(key, value)?,
        "notify.after" => settings.notify_after = notify_after(key, value)?,
        _ => {
            let action = key
                .strip_prefix("keys.")
                .and_then(Action::parse)
                .ok_or_else(|| CoreError::UnknownSetting(key.to_owned()))?;
            let chord = Chord::parse(value)?;
            settings.keymap.bind(action, chord)?;
        }
    }
    Ok(())
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
}
