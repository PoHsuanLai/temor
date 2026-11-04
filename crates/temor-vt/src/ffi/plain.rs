//! A terminal's text, plain.

use super::format::Formatter;
use super::terminal::Terminal;
use crate::{TextFormat, TextSpan, VtError};

impl Terminal {
    /// The plain text of everything kept (scrollback and screen), soft wraps joined and trailing
    /// blanks trimmed.
    pub fn plain_text(&self) -> Result<String, VtError> {
        Formatter::new(TextFormat::Plain).format(self, TextSpan::Screen)
    }
}
