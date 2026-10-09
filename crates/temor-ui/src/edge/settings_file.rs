//! Reads the settings file. A missing file means the defaults; a file that cannot be read is an
//! error the window shows. Bad lines are kept as issues and the rest of the file applies.

use std::path::Path;

use temor_core::terminal::settings::{ParsedSettings, parse_settings};

use crate::error::UiError;

/// The settings at `path`, over the defaults.
pub fn load_settings(path: &Path) -> Result<ParsedSettings, UiError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(parse_settings(&text)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(parse_settings("")),
        Err(source) => Err(UiError::SettingsRead(source)),
    }
}
