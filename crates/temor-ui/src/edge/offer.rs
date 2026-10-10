//! The shell offer's reads and writes outside the window (PLAN P17): the line that loads temor's
//! integration into the inner shell, and the "never for" choice kept in the settings file.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::path::Path;

use temor_core::offer::Program;
use temor_ssh::{InnerLoadLine, InnerShell, LoadToken, SshError, probe_line};

/// The one line a Yes types first: it makes the inner shell name itself.
pub fn probe_text() -> String {
    probe_line()
}

/// The lines to type into the pane once `shell` has named itself. Built from the scripts temor
/// ships, with a name nobody can guess for the one file they write on the host.
pub fn load_line(shell: InnerShell) -> Result<InnerLoadLine, SshError> {
    InnerLoadLine::build_for(shell, &temor_shell::integration_scripts(), &fresh_token())
}

/// Two words the standard library seeds randomly for each hasher.
fn fresh_token() -> LoadToken {
    let word = || RandomState::new().build_hasher().finish();
    LoadToken::from_words(word(), word())
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
