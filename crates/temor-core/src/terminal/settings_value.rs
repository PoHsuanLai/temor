//! The typed values of the pane's settings, and how each is read from its text. Parsing happens
//! here, once, at the boundary: the rest of the app holds these types and needs no checks.

use ds_core::word::Word;

use crate::error::CoreError;

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

/// The cursor's shape. The program may still change it (DECSCUSR); this is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
#[word(case = snake)]
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
pub fn font_points(key: &str, value: &str) -> Result<FontPoints, CoreError> {
    value
        .parse::<f32>()
        .ok()
        .and_then(FontPoints::new)
        .ok_or_else(|| bad(key, value))
}

/// Reads a scrollback length: a whole number from 0 to one million.
pub fn scrollback(key: &str, value: &str) -> Result<ScrollbackLines, CoreError> {
    value
        .parse::<u32>()
        .ok()
        .and_then(ScrollbackLines::new)
        .ok_or_else(|| bad(key, value))
}

/// Reads a cursor style: block, bar or underline.
pub fn cursor(key: &str, value: &str) -> Result<CursorStyle, CoreError> {
    CursorStyle::parse(value).ok_or_else(|| bad(key, value))
}

/// Reads a font family: "system" or a family name.
pub fn font_family(key: &str, value: &str) -> Result<FontChoice, CoreError> {
    match value.trim() {
        "" => Err(bad(key, value)),
        "system" => Ok(FontChoice::SystemMonospace),
        name => Ok(FontChoice::Named(name.to_owned())),
    }
}

/// Reads a shell: "login" or a program name or path.
pub fn shell(key: &str, value: &str) -> Result<ShellChoice, CoreError> {
    match value.trim() {
        "" => Err(bad(key, value)),
        "login" => Ok(ShellChoice::Login),
        program => Ok(ShellChoice::Program(program.to_owned())),
    }
}

fn bad(key: &str, value: &str) -> CoreError {
    CoreError::BadSettingValue {
        key: key.to_owned(),
        value: value.to_owned(),
    }
}
