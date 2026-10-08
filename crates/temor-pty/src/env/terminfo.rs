//! Picking `TERM`: `xterm-ghostty` when its terminfo entry can be found, without `infocmp`.

use std::path::{Path, PathBuf};

use super::model::ChildEnv;
use ds_core::word::Word;

/// The `TERM` values temor sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TermName {
    /// Ghostty's own entry, which describes what libghostty-vt really does.
    XtermGhostty,
    /// The fallback every system has.
    #[word(slug = "xterm-256color")]
    Xterm256Color,
}

/// Where ncurses looks when neither `TERMINFO` nor `TERMINFO_DIRS` says otherwise, across
/// Linux distributions, the BSDs and macOS. Callers pass this (or their own list) to
/// [`terminfo_dirs`].
pub const SYSTEM_TERMINFO_DIRS: &[&str] = &[
    "/etc/terminfo",
    "/lib/terminfo",
    "/usr/share/terminfo",
    "/usr/lib/terminfo",
    "/usr/local/share/terminfo",
    "/usr/share/lib/terminfo",
    "/opt/homebrew/share/terminfo",
];

/// The directories to search, in ncurses' order: `$TERMINFO`, `$HOME/.terminfo`, then
/// `$TERMINFO_DIRS` (an empty entry stands for the system list), else the system list.
pub fn terminfo_dirs(env: &ChildEnv, system: &[&str]) -> Vec<PathBuf> {
    let system_dirs = || system.iter().map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = env.get("TERMINFO").filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(dir));
    }
    if let Some(home) = env.get("HOME").filter(|h| !h.is_empty()) {
        dirs.push(Path::new(home).join(".terminfo"));
    }
    match env.get("TERMINFO_DIRS") {
        Some(list) => {
            for entry in std::env::split_paths(list) {
                if entry.as_os_str().is_empty() {
                    dirs.extend(system_dirs());
                } else {
                    dirs.push(entry);
                }
            }
        }
        None => dirs.extend(system_dirs()),
    }
    dirs
}

/// `xterm-ghostty` if any of `dirs` holds its entry (`x/xterm-ghostty`, or macOS's hashed
/// `78/xterm-ghostty`), else `xterm-256color`.
pub fn find_term(dirs: &[PathBuf]) -> TermName {
    let name = TermName::XtermGhostty.slug();
    let found = dirs
        .iter()
        .any(|dir| dir.join("x").join(name).is_file() || dir.join("78").join(name).is_file());
    match found {
        true => TermName::XtermGhostty,
        false => TermName::Xterm256Color,
    }
}
