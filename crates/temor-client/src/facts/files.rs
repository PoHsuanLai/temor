//! The facts that come from files: the Python virtualenv, and the versions of Node, Python, Ruby
//! and Go a project asks for. Anything unreadable is left out.

use std::io::Read;
use std::path::Path;

use temor_core::facts::{EnvName, FolderSource, Runtimes, runtimes_in, venv_name};

use temor_wire::message::{MOST_FACT_BYTES, MOST_FACT_PARENTS};

use super::FactsEnv;

/// The virtualenv: the environment's `VIRTUAL_ENV` if the pane has one, else a `.venv` folder
/// in the folder.
pub(super) fn read_venv(dir: &Path, env: &FactsEnv) -> Option<EnvName> {
    if let Some(path) = env.virtual_env.as_deref().filter(|p| !p.is_empty()) {
        return venv_name(path);
    }
    let local = dir.join(".venv");
    match local.is_dir() {
        true => venv_name(&local.to_string_lossy()),
        false => None,
    }
}

/// This computer's disk.
struct OnDisk;

impl FolderSource for OnDisk {
    fn read(&self, folder: &Path, file: &str) -> Option<String> {
        // The same cap the host's helper reads under, so a big file gives a chip on neither.
        let opened = std::fs::File::open(folder.join(file)).ok()?;
        let mut text = String::new();
        opened
            .take(MOST_FACT_BYTES + 1)
            .read_to_string(&mut text)
            .ok()?;
        (text.len() as u64 <= MOST_FACT_BYTES).then_some(text)
    }

    fn is_root(&self, folder: &Path) -> bool {
        folder.join(".git").exists()
    }
}

/// The versions from the nearest files, looking in the folder and then each parent up to the
/// repository root (the folder that holds `.git`); the rules are `temor_core::facts::runtimes_in`.
pub(super) fn read_runtimes(dir: &Path) -> Runtimes {
    runtimes_in(dir, &OnDisk, MOST_FACT_PARENTS)
}
