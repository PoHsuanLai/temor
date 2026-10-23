//! The pane's settings file text: one `key = value` per line, `#` lines are comments. A bad
//! line is reported and skipped, so one typo never costs the rest of the file (PLAN §14).

use super::start::{PagerChoice, StartIn};
use super::value::{
    CursorBlinking, CursorStyle, FontChoice, FontPoints, LinePercent, NotifyAfter, ScrollbackLines,
    ShellChoice, cursor, cursor_blink, font_family, font_points, line_percent, notify_after,
    offer_command, scrollback, shell,
};
use crate::error::SettingsError;
use crate::keys::Action;
use chordkit::Overrides;

/// Everything the pane reads from its settings.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalSettings {
    pub font: FontChoice,
    pub font_size: FontPoints,
    /// How tall a line is (`font.line_height`, percent of the font's own line).
    pub line_height: LinePercent,
    pub scrollback: ScrollbackLines,
    pub cursor: CursorStyle,
    /// Whether the cursor blinks (`cursor.blink = on | off`).
    pub cursor_blink: CursorBlinking,
    pub shell: ShellChoice,
    /// Where a new tab and a split start (`shell.start_in` in `settings.toml`).
    pub start_in: StartIn,
    /// Where paged output goes (`shell.pager`).
    pub pager: PagerChoice,
    /// Whether new terminals run in the background daemon, so quitting leaves them running
    /// (`shell.keep_running`).
    pub keep_running: bool,
    /// The person's key changes, in chordkit's line form (`temor.split-right = Primary+Alt+N`).
    /// The keymap itself is built at the app's edge from the desktop's conventions and these.
    pub keys: Overrides,
    /// When a finished command is worth a notice while the pane is unfocused (PLAN §8.1).
    pub notify_after: NotifyAfter,
    /// Commands added to the shell offer's list (`offer.add = kubectl debug -it`), each as the
    /// first words of a command line. Repeat the line for more.
    pub offer_add: Vec<String>,
    /// Commands taken off the shell offer's list (`offer.remove = su`), including the fixed ones.
    pub offer_remove: Vec<String>,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font: FontChoice::default(),
            font_size: FontPoints::DEFAULT,
            line_height: LinePercent::DEFAULT,
            scrollback: ScrollbackLines::DEFAULT,
            cursor: CursorStyle::default(),
            cursor_blink: CursorBlinking::default(),
            shell: ShellChoice::default(),
            start_in: StartIn::default(),
            pager: PagerChoice::default(),
            keep_running: false,
            keys: Overrides::default(),
            notify_after: NotifyAfter::DEFAULT,
            offer_add: Vec::new(),
            offer_remove: Vec::new(),
        }
    }
}

/// The settings and the lines that were refused, each with its 1-based line number.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSettings {
    pub settings: TerminalSettings,
    pub issues: Vec<(usize, SettingsError)>,
}

/// Reads settings text over the defaults. Every problem is kept in `issues`; the rest applies.
///
/// A line whose left side names a key action (`copy`, `temor.split-right`) is a key line. Those
/// lines go to chordkit's reader whole, in place, so its problems carry the file's line numbers.
pub fn parse_settings(text: &str) -> ParsedSettings {
    let mut settings = TerminalSettings::default();
    let mut issues = Vec::new();
    let mut key_text = String::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let key_line = is_key_line(line);
        if key_line {
            key_text.push_str(raw);
        }
        key_text.push('\n');
        if line.is_empty() || line.starts_with('#') || key_line {
            continue;
        }
        if let Err(err) = apply_line(&mut settings, line) {
            issues.push((index + 1, err));
        }
    }
    let (keys, problems) = Overrides::parse(&key_text);
    settings.keys = keys;
    issues.extend(
        problems
            .into_iter()
            .map(|problem| (problem.line, SettingsError::Key(problem.what))),
    );
    issues.sort_by_key(|(line, _)| *line);
    ParsedSettings { settings, issues }
}

/// Whether `line` is a key line: its left side is a standard action (`copy`) or one of temor's
/// own (`temor.split-right`). Another app's action id is not, and is reported as unknown.
fn is_key_line(line: &str) -> bool {
    let Some((key, _)) = line.split_once('=') else {
        return false;
    };
    match key.trim().parse::<chordkit::Action>() {
        Ok(chordkit::Action::Standard(_)) => true,
        Ok(other) => Action::from_chordkit(&other).is_some(),
        Err(_) => false,
    }
}

fn apply_line(settings: &mut TerminalSettings, line: &str) -> Result<(), SettingsError> {
    let (key, raw) = line
        .split_once('=')
        .ok_or_else(|| SettingsError::UnknownSetting(line.to_owned()))?;
    let key = key.trim();
    let value = unquote(raw.trim());
    match key {
        "font.family" => settings.font = font_family(key, value)?,
        "font.size" => settings.font_size = font_points(key, value)?,
        "font.line_height" => settings.line_height = line_percent(key, value)?,
        "scrollback.lines" => settings.scrollback = scrollback(key, value)?,
        "cursor.style" => settings.cursor = cursor(key, value)?,
        "cursor.blink" => settings.cursor_blink = cursor_blink(key, value)?,
        "shell.program" => settings.shell = shell(key, value)?,
        "notify.after" => settings.notify_after = notify_after(key, value)?,
        "offer.add" => settings.offer_add.push(offer_command(key, value)?),
        "offer.remove" => settings.offer_remove.push(offer_command(key, value)?),
        old if old.starts_with("keys.") => {
            return Err(SettingsError::Key(format!(
                "{old} is the old way to write a key; name the action itself, for example `copy = Ctrl+Shift+C` or `temor.zoom = none`"
            )));
        }
        _ => return Err(SettingsError::UnknownSetting(key.to_owned())),
    }
    Ok(())
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
}
