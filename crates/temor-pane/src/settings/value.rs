//! The typed values of the pane's settings, and how each is read from its text. Parsing happens
//! here, once, at the boundary: the rest of the app holds these types and needs no checks.

use crate::error::SettingsError;

/// Font size in points (the logical size the renderer scales).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct FontPoints(f32);

impl FontPoints {
    pub const MIN: f32 = 6.0;
    pub const MAX: f32 = 72.0;
    /// The size when the settings say nothing.
    pub const DEFAULT: Self = Self(13.0);

    /// The size in points, `None` when outside 6 to 72.
    pub fn new(points: f32) -> Option<Self> {
        (points.is_finite() && (Self::MIN..=Self::MAX).contains(&points)).then_some(Self(points))
    }

    pub fn points(self) -> f32 {
        self.0
    }
}

/// How many lines of scrollback are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScrollbackLines(u32);

impl ScrollbackLines {
    pub const MAX: u32 = 1_000_000;
    /// The lines kept when the settings say nothing.
    pub const DEFAULT: Self = Self(10_000);

    /// The count, `None` above one million.
    pub fn new(lines: u32) -> Option<Self> {
        (lines <= Self::MAX).then_some(Self(lines))
    }

    pub fn lines(self) -> u32 {
        self.0
    }
}

impl CursorStyle {
    /// The style written as `slug` in settings text (`bar`), or `None`.
    pub fn parse(slug: &str) -> Option<Self> {
        match slug {
            "block" => Some(Self::Block),
            "bar" => Some(Self::Bar),
            "underline" => Some(Self::Underline),
            _ => None,
        }
    }
}

/// The cursor's shape. The program may still change it (DECSCUSR); this is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorStyle {
    #[default]
    Block,
    Bar,
    Underline,
}

/// Whether the cursor blinks. `On`: it blinks unless the program asks for a steady one (DECSCUSR,
/// DEC mode 12), as Ghostty's does. `Off`: it never blinks, whatever the program asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorBlinking {
    #[default]
    On,
    Off,
}

/// Which font the grid uses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum FontChoice {
    /// The system's monospace face.
    #[default]
    SystemMonospace,
    /// A family by name, as fontique finds it.
    Named(String),
}

/// Which program a new pane starts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum ShellChoice {
    /// The person's login shell.
    #[default]
    Login,
    /// A program by path or name.
    Program(String),
}

/// Reads a font size: a number from 6 to 72.
pub fn font_points(key: &str, value: &str) -> Result<FontPoints, SettingsError> {
    value
        .parse::<f32>()
        .ok()
        .and_then(FontPoints::new)
        .ok_or_else(|| bad(key, value))
}

/// Reads a scrollback length: a whole number from 0 to one million.
pub fn scrollback(key: &str, value: &str) -> Result<ScrollbackLines, SettingsError> {
    value
        .parse::<u32>()
        .ok()
        .and_then(ScrollbackLines::new)
        .ok_or_else(|| bad(key, value))
}

/// Reads a cursor style: block, bar or underline.
pub fn cursor(key: &str, value: &str) -> Result<CursorStyle, SettingsError> {
    CursorStyle::parse(value).ok_or_else(|| bad(key, value))
}

/// Reads whether the cursor blinks: on or off.
pub fn cursor_blink(key: &str, value: &str) -> Result<CursorBlinking, SettingsError> {
    match value.trim() {
        "on" => Ok(CursorBlinking::On),
        "off" => Ok(CursorBlinking::Off),
        _ => Err(bad(key, value)),
    }
}

/// Reads how long a command must run before its finish is notified: a number of seconds, where
/// 0 or "off" turns the notices off.
pub fn notify_after(key: &str, value: &str) -> Result<NotifyAfter, SettingsError> {
    match value.trim() {
        "off" => Ok(NotifyAfter::Off),
        text => match text.parse::<u32>() {
            Ok(0) => Ok(NotifyAfter::Off),
            Ok(seconds) if seconds <= NOTIFY_AFTER_MAX_SECONDS => Ok(NotifyAfter::Seconds(seconds)),
            _ => Err(bad(key, value)),
        },
    }
}

/// Reads a command for the shell offer's lists: its first words, single-spaced. Empty, over 64
/// characters, or holding a control character is refused.
pub fn offer_command(key: &str, value: &str) -> Result<String, SettingsError> {
    let words = value.split_whitespace().collect::<Vec<_>>().join(" ");
    match words.is_empty() || words.chars().count() > 64 || words.chars().any(char::is_control) {
        true => Err(bad(key, value)),
        false => Ok(words),
    }
}

/// The longest threshold a setting may name: a day.
const NOTIFY_AFTER_MAX_SECONDS: u32 = 86_400;

/// Reads a font family: "system" or a family name.
pub fn font_family(key: &str, value: &str) -> Result<FontChoice, SettingsError> {
    match value.trim() {
        "" => Err(bad(key, value)),
        "system" => Ok(FontChoice::SystemMonospace),
        name => Ok(FontChoice::Named(name.to_owned())),
    }
}

/// Reads a shell: "login" or a program name or path.
pub fn shell(key: &str, value: &str) -> Result<ShellChoice, SettingsError> {
    match value.trim() {
        "" => Err(bad(key, value)),
        "login" => Ok(ShellChoice::Login),
        program => Ok(ShellChoice::Program(program.to_owned())),
    }
}

fn bad(key: &str, value: &str) -> SettingsError {
    SettingsError::BadSettingValue {
        key: key.to_owned(),
        value: value.to_owned(),
    }
}

/// How long a command must run before its finish is worth a notice. `Off` never notifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotifyAfter {
    Off,
    /// A command running at least this many seconds.
    Seconds(u32),
}

impl NotifyAfter {
    /// The threshold when the settings say nothing: ten seconds (PLAN §6, the notify setting).
    pub const DEFAULT: Self = Self::Seconds(10);

    /// The threshold in milliseconds; `None` when off.
    pub fn threshold_ms(self) -> Option<u64> {
        match self {
            Self::Off => None,
            Self::Seconds(s) => Some(u64::from(s) * 1000),
        }
    }
}
