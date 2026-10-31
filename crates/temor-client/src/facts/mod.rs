//! Reading a folder's facts (PLAN P16): the environment and the folder for the Python
//! virtualenv, and the version files for Node, Python, Ruby and Go. Read on the facts' own line
//! ([`read_later`], [`ask_host_later`]), never on the window's thread. A fact that cannot be read is left out; nothing here
//! reports an error. temor runs no git of its own: what a repository says is a plugin's to tell.

mod files;
pub mod host;

use std::path::Path;
use std::sync::Arc;

use temor_core::facts::Facts;
use temor_wire::ids::SessionId;

use crate::remote::conn::Conn;
use crate::serial::Serial;
use files::{read_runtimes, read_venv};

/// Reads `dir`'s facts on the facts' own thread, after the reads asked before, and hands them
/// to `done`. False when that thread could not start: `done` is never called.
pub fn read_later(dir: String, env: FactsEnv, done: impl FnOnce(Facts) + Send + 'static) -> bool {
    static LINE: Serial = Serial::new("temor-facts");
    LINE.start(move || done(read_facts(&dir, &env)))
}

/// Asks the helper on `conn` for the facts of `session`'s folder on a thread of its own (a host
/// that is slow to answer holds up no local read), and hands them to `done`: none when it did not
/// answer. False when that thread could not start: `done` is never called.
pub fn ask_host_later(
    (conn, session): (Arc<Conn>, SessionId),
    done: impl FnOnce(Option<Facts>) + Send + 'static,
) -> bool {
    static LINE: Serial = Serial::new("temor-facts-host");
    LINE.start(move || {
        let sent = conn.client().folder_facts(session).ok();
        done(sent.map(|sent| host::facts_from_host(&sent)));
    })
}

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
mod tests;
