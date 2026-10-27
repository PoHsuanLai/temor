//! The Run row's `PATH` lookup (PLAN §7 rule 2): a lookup, not a guess.

use std::ffi::OsString;

use temor_client::model::bar::PathResolver;

/// Finds programs on a `PATH`-style list, as the shell would for a bare name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathLookup {
    path: OsString,
}

impl PathLookup {
    /// A lookup over `path` (the `PATH` a new shell starts with).
    pub fn new(path: OsString) -> PathLookup {
        PathLookup { path }
    }
}

impl PathResolver for PathLookup {
    fn resolves(&self, program: &str) -> bool {
        // A path is not a lookup: `./run` and `/bin/ls` do not make a Run row.
        !program.is_empty()
            && !program.contains('/')
            && temor_panes::pty::find_program(program, &self.path).is_some()
    }
}
