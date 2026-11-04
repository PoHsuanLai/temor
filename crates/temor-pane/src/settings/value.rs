//! The typed values of the pane's settings, and how each is read from its text. Parsing happens
//! here, once, at the boundary: the rest of the app holds these types and needs no checks. The
//! numbers read from the settings file as they are written there: a value outside its limit is
//! refused as it is read, with words that say the limit.

use serde::{Deserialize, Serialize};

use crate::error::SettingsError;

/// Font size in points (the logical size the renderer scales). `font.size` in the file.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
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

impl Default for FontPoints {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<f32> for FontPoints {
    type Error = String;

    fn try_from(points: f32) -> Result<Self, String> {
        Self::new(points)
            .ok_or_else(|| format!("a size from {} to {} points", Self::MIN, Self::MAX))
    }
}

impl From<FontPoints> for f32 {
    fn from(size: FontPoints) -> Self {
        size.0
    }
}

/// How tall a terminal line is, in percent of the font's own line: 100 keeps it, 120 opens it by
/// a fifth. `font.line_height` in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct LinePercent(u32);

impl LinePercent {
    pub const MIN: u32 = 80;
    pub const MAX: u32 = 200;
    /// The height when the settings say nothing.
    pub const DEFAULT: Self = Self(100);

    /// The percentage, `None` outside 80 to 200.
    pub fn new(percent: u32) -> Option<Self> {
        (Self::MIN..=Self::MAX)
            .contains(&percent)
            .then_some(Self(percent))
    }

    pub fn percent(self) -> u32 {
        self.0
    }

    /// The same as a multiple of the font's line.
    pub fn factor(self) -> f32 {
        self.0 as f32 / 100.0
    }
}

impl Default for LinePercent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u32> for LinePercent {
    type Error = String;

    fn try_from(percent: u32) -> Result<Self, String> {
        Self::new(percent).ok_or_else(|| format!("a percent from {} to {}", Self::MIN, Self::MAX))
    }
}

impl From<LinePercent> for u32 {
    fn from(line: LinePercent) -> Self {
        line.0
    }
}

/// How many lines of scrollback are kept. `scrollback.lines` in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
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

impl Default for ScrollbackLines {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u32> for ScrollbackLines {
    type Error = String;

    fn try_from(lines: u32) -> Result<Self, String> {
        Self::new(lines).ok_or_else(|| format!("at most {} lines", Self::MAX))
    }
}

impl From<ScrollbackLines> for u32 {
    fn from(lines: ScrollbackLines) -> Self {
        lines.0
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

/// Reads a line height: a whole percent from 80 to 200.
pub fn line_percent(key: &str, value: &str) -> Result<LinePercent, SettingsError> {
    value
        .trim_end_matches('%')
        .trim()
        .parse::<u32>()
        .ok()
        .and_then(LinePercent::new)
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
pub fn cursor_blink(key: &str, value: &str) -> Result<bool, SettingsError> {
    match value.trim() {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(bad(key, value)),
    }
}

/// Reads how long a command must run before its finish is notified: a number of seconds, where
/// 0 or "off" turns the notices off.
pub fn notify_after(key: &str, value: &str) -> Result<NotifyAfter, SettingsError> {
    match value.trim() {
        "off" => Ok(NotifyAfter::Off),
        text => text
            .parse::<u32>()
            .ok()
            .and_then(|seconds| NotifyAfter::try_from(seconds).ok())
            .ok_or_else(|| bad(key, value)),
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
pub const NOTIFY_AFTER_MAX_SECONDS: u32 = 86_400;

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
/// `notify.after` in the file: seconds, 0 for off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
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

impl Default for NotifyAfter {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u32> for NotifyAfter {
    type Error = String;

    fn try_from(seconds: u32) -> Result<Self, String> {
        match seconds {
            0 => Ok(Self::Off),
            s if s <= NOTIFY_AFTER_MAX_SECONDS => Ok(Self::Seconds(s)),
            _ => Err(format!("at most {NOTIFY_AFTER_MAX_SECONDS} seconds")),
        }
    }
}

impl From<NotifyAfter> for u32 {
    fn from(after: NotifyAfter) -> Self {
        match after {
            NotifyAfter::Off => 0,
            NotifyAfter::Seconds(seconds) => seconds,
        }
    }
}
