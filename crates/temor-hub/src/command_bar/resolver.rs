//! The Run row's `PATH` lookup (PLAN §7 rule 2): a lookup, not a guess.

use std::ffi::OsString;

/// Finds a program on the person's `PATH`: the bar's Run row exists only when this says the
/// first word is there. The real one looks; tests pass a fixed table.
pub trait PathResolver {
    /// Whether `program` is a command the shell would find.
    fn resolves(&self, program: &str) -> bool;

    /// The first word of `query`, when the shell would find it.
    fn runnable<'q>(&self, query: &'q str) -> Option<&'q str> {
        let program = query.split_whitespace().next()?;
        self.resolves(program).then_some(program)
    }
}

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
