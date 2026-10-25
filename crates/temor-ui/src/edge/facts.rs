//! Reading a folder's facts (PLAN P16): the environment and the folder for the Python
//! virtualenv, and the version files for Node, Python, Ruby and Go. Runs on a worker thread the
//! hub starts, never on the window's. A fact that cannot be read is left out; nothing here
//! reports an error. temor runs no git of its own: what a repository says is a plugin's to tell.

use std::path::Path;

use temor_core::facts::Facts;

use super::facts_files::{read_runtimes, read_venv};

/// What `read_facts` needs besides the folder.
#[derive(Debug, Clone, Default)]
pub struct FactsEnv {
    /// `VIRTUAL_ENV` from the environment the pane's shell started with, if it had one.
    pub virtual_env: Option<String>,
}

/// Everything known about `dir`.
pub fn read_facts(dir: &str, env: &FactsEnv) -> Facts {
    let runtimes = read_runtimes(Path::new(dir));
    Facts {
        venv: read_venv(Path::new(dir), env),
        node: runtimes.node,
        python: runtimes.python,
        ruby: runtimes.ruby,
        go: runtimes.go,
    }
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
