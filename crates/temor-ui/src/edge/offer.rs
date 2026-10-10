//! The shell offer's reads and writes outside the window (PLAN P17): the line that loads temor's
//! integration into the inner shell, and the "never for" choice kept in the settings file.

use std::io::Write;
use std::path::Path;

use temor_core::offer::Program;
use temor_ssh::{InnerLoadLine, SshError};

/// The line to type into the pane after a Yes. Built from the scripts temor ships.
pub fn load_line() -> Result<InnerLoadLine, SshError> {
    InnerLoadLine::build(&temor_shell::integration_scripts())
}

/// Adds `offer.remove = <program>` to the settings file at `path`, so the choice survives a
/// restart. The file and its folder are made when missing; the person's other lines are kept.
pub fn remember_never(path: &Path, program: &Program) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let lead = match existing.is_empty() || existing.ends_with('\n') {
        true => "",
        false => "\n",
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{lead}offer.remove = {program}")
}
