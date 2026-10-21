//! The numbers of temor's settings, as the file holds them. Each keeps `temor-pane`'s limit: a
//! value outside it is refused when the file is read (ds-settings then uses the key's default
//! and reports the key), so the rest of the app never checks again.

use serde::{Deserialize, Serialize};
use temor_pane::settings::{
    FontPoints, LinePercent, NOTIFY_AFTER_MAX_SECONDS, NotifyAfter, ScrollbackLines,
};

/// `font.size`: points, from 6 to 72.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct FontSize(pub FontPoints);

impl Default for FontSize {
    fn default() -> Self {
        Self(FontPoints::DEFAULT)
    }
}

impl TryFrom<f32> for FontSize {
    type Error = String;

    fn try_from(points: f32) -> Result<Self, String> {
        FontPoints::new(points).map(Self).ok_or_else(|| {
            format!(
                "a size from {} to {} points",
                FontPoints::MIN,
                FontPoints::MAX
            )
        })
    }
}

impl From<FontSize> for f32 {
    fn from(size: FontSize) -> Self {
        size.0.points()
    }
}

/// `font.line_height`: percent of the font's own line, from 80 to 200.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct LineSpacing(pub LinePercent);

impl Default for LineSpacing {
    fn default() -> Self {
        Self(LinePercent::DEFAULT)
    }
}

impl TryFrom<u32> for LineSpacing {
    type Error = String;

    fn try_from(percent: u32) -> Result<Self, String> {
        LinePercent::new(percent).map(Self).ok_or_else(|| {
            format!(
                "a percent from {} to {}",
                LinePercent::MIN,
                LinePercent::MAX
            )
        })
    }
}

impl From<LineSpacing> for u32 {
    fn from(line: LineSpacing) -> Self {
        line.0.percent()
    }
}

/// `scrollback.lines`: how many lines are kept, up to a million.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct ScrollbackSize(pub ScrollbackLines);

impl Default for ScrollbackSize {
    fn default() -> Self {
        Self(ScrollbackLines::DEFAULT)
    }
}

impl TryFrom<u32> for ScrollbackSize {
    type Error = String;

    fn try_from(lines: u32) -> Result<Self, String> {
        ScrollbackLines::new(lines)
            .map(Self)
            .ok_or_else(|| format!("at most {} lines", ScrollbackLines::MAX))
    }
}

impl From<ScrollbackSize> for u32 {
    fn from(size: ScrollbackSize) -> Self {
        size.0.lines()
    }
}

/// `notify.after`: seconds a command must run before its finish is announced; 0 turns the
/// announcements off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct NotifySeconds(pub NotifyAfter);

impl Default for NotifySeconds {
    fn default() -> Self {
        Self(NotifyAfter::DEFAULT)
    }
}

impl TryFrom<u32> for NotifySeconds {
    type Error = String;

    fn try_from(seconds: u32) -> Result<Self, String> {
        match seconds {
            0 => Ok(Self(NotifyAfter::Off)),
            s if s <= NOTIFY_AFTER_MAX_SECONDS => Ok(Self(NotifyAfter::Seconds(s))),
            _ => Err(format!("at most {NOTIFY_AFTER_MAX_SECONDS} seconds")),
        }
    }
}

impl From<NotifySeconds> for u32 {
    fn from(after: NotifySeconds) -> Self {
        match after.0 {
            NotifyAfter::Off => 0,
            NotifyAfter::Seconds(seconds) => seconds,
        }
    }
}
