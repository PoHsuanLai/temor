//! The shell offer's reads and writes outside the window (PLAN P17): the line that loads temor's
//! integration into the inner shell, and the "never for" choice kept in `settings.toml`.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;

use ds_settings::Store;
use temor_client::model::offer::Program;
use temor_client::ssh::{InnerLoadLine, InnerShell, LoadToken, SshError, probe_line};

use crate::error::UiError;
use crate::settings::TerminalDoc;

/// The one line a Yes types first: it makes the inner shell name itself.
pub fn probe_text() -> String {
    probe_line()
}

/// The lines to type into the pane once `shell` has named itself. Built from the scripts temor
/// ships, with a name nobody can guess for the one file they write on the host.
pub fn load_line(shell: InnerShell) -> Result<InnerLoadLine, SshError> {
    InnerLoadLine::build_for(
        shell,
        &temor_panes::shell::integration_scripts(),
        &fresh_token(),
    )
}

/// Two words the standard library seeds randomly for each hasher.
fn fresh_token() -> LoadToken {
    let word = || RandomState::new().build_hasher().finish();
    LoadToken::from_words(word(), word())
}

/// Adds `program` to the shell offer's "never offer for" list in `settings.toml`, so the choice
/// survives a restart. The file and its folder are made when missing; the person's other
/// settings are kept. Returns where an unreadable file was set aside to make room, if one was.
pub fn remember_never(store: &Store, program: &Program) -> Result<Option<PathBuf>, UiError> {
    let mut doc = store.load::<TerminalDoc>().value;
    let name = program.to_string();
    if !doc.offer.remove.contains(&name) {
        doc.offer.remove.push(name);
    }
    let saved = store.save(&doc).map_err(UiError::SettingsWrite)?;
    Ok(saved.kept_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::Ambient;

    #[test]
    fn a_never_saved_over_an_unreadable_file_says_where_the_old_one_went() {
        let scratch = tempfile::tempdir().expect("scratch");
        let ambient = Ambient {
            home: scratch.path().join("home"),
            env: temor_panes::shell::ChildEnv::new(),
            config_dir: scratch.path().join("config/temor"),
            runtime_dir: scratch.path().join("runtime"),
        };
        let store = ambient.settings_store();
        let path = store.path::<TerminalDoc>().expect("a settings path");
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
        std::fs::write(&path, "offer = [\n").expect("written");
        let program = Program("vim".to_owned());
        let kept = remember_never(&store, &program)
            .expect("saved")
            .expect("the old file was set aside");
        assert_eq!(std::fs::read_to_string(&kept).expect("kept"), "offer = [\n");
        assert_eq!(store.load::<TerminalDoc>().value.offer.remove, ["vim"]);
        assert_eq!(remember_never(&store, &program).expect("saved"), None);
    }
}
