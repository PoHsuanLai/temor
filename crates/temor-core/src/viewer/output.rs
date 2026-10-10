//! The paths a command printed: the lines of an `ls`, `find` or `rg` that name files.

use std::path::{Path, PathBuf};

use super::token::candidates;

/// The most paths one output gives. A listing longer than this is cut: the arrows walk a few
/// hundred files, not a whole tree.
pub const MAX_PATHS: usize = 500;

/// Where `candidate` is, from the folder `cwd` is. An absolute path is itself; a relative one
/// needs a folder to start from. Joins only: `..` and `.` stay for the viewer's own tidying.
pub fn resolve(cwd: Option<&str>, candidate: &str) -> Option<PathBuf> {
    if candidate.is_empty() || candidate.chars().any(char::is_control) {
        return None;
    }
    let path = Path::new(candidate);
    match path.is_absolute() {
        true => Some(path.to_path_buf()),
        false => cwd
            .filter(|cwd| !cwd.is_empty())
            .map(|cwd| Path::new(cwd).join(path)),
    }
}

/// The first of `token`'s candidates that `is_file` accepts, resolved from `cwd`.
fn first_file(token: &str, cwd: Option<&str>, is_file: &impl Fn(&Path) -> bool) -> Option<PathBuf> {
    candidates(token)
        .iter()
        .filter_map(|candidate| resolve(cwd, candidate))
        .find(|path| is_file(path))
}

/// The files `output` lists, in the order they were printed, each once, at most [`MAX_PATHS`].
/// A line that names a file as a whole is that file (a name with spaces in it, `find`'s lines,
/// `rg`'s `path:line:text`); otherwise each of its words that names a file is one (`ls`'s
/// columns, `ls -l`'s last). `is_file` is the edge's check that a path exists and is a file.
pub fn output_paths(
    output: &str,
    cwd: Option<&str>,
    is_file: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut add = |path: PathBuf| {
        if found.len() < MAX_PATHS && !found.contains(&path) {
            found.push(path);
        }
    };
    for line in output.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match first_file(line, cwd, &is_file) {
            Some(path) => add(path),
            None => line
                .split_whitespace()
                .filter_map(|word| first_file(word, cwd, &is_file))
                .for_each(&mut add),
        }
    }
    found
}
