//! The pane's settings text: one `key = value` per line, and the value each setting takes (PLAN §6,
//! §14). Parsing is pure: the caller reads the file and reports the lines that were refused.

mod parse;
#[cfg(test)]
mod tests;
mod value;

pub use parse::{ParsedSettings, TerminalSettings, parse_settings};
pub use value::{CursorStyle, FontChoice, FontPoints, NotifyAfter, ScrollbackLines, ShellChoice};
