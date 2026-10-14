//! The shell offer's reads and writes outside the window (PLAN P17): the line that loads temor's
//! integration into the inner shell, and the "never for" choice kept in `settings.toml`.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use ds_settings::Store;
use temor_core::offer::Program;
use temor_ssh::{InnerLoadLine, InnerShell, LoadToken, SshError, probe_line};

use crate::error::UiError;
use crate::settings::TerminalDoc;

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

/// Adds `program` to the shell offer's "never offer for" list in `settings.toml`, so the choice
/// survives a restart. The file and its folder are made when missing; the person's other
/// settings are kept.
pub fn remember_never(store: &Store, program: &Program) -> Result<(), UiError> {
    let mut doc = store.load::<TerminalDoc>().value;
    let name = program.to_string();
    if !doc.offer.remove.contains(&name) {
        doc.offer.remove.push(name);
    }
    store.save(&doc).map_err(UiError::SettingsWrite)
}
