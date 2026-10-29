//! The pane's settings text: one `key = value` per line, and the value each setting takes (PLAN §6,
//! §14). Parsing is pure: the caller reads the file and reports the lines that were refused.

mod parse;
mod start;
#[cfg(test)]
mod tests;
mod value;

pub use parse::{ParsedSettings, TerminalSettings, parse_settings};
pub use start::{PagerChoice, StartIn};
pub use value::{
    CursorStyle, FontChoice, FontPoints, LinePercent, NOTIFY_AFTER_MAX_SECONDS, NotifyAfter,
    ScrollbackLines, ShellChoice, font_family, line_percent, offer_command, shell,
};
