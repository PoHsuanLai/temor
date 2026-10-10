//! Setting a new worktree up from project facts, never from the agent: ignored files the
//! project needs (`.env`, local config) are copied from a gitignore-style include list kept in
//! the repo, then the Space profile's setup command runs as a block in a pane.

use crate::vocab::text::{CommandText, RelPath};

/// The file name the include list is read from. If Claude Code's desktop app really reads a
/// file of this name (unverified, PLAN §8.2) the two share it.
pub const INCLUDE_FILE: &str = ".worktreeinclude";

/// The patterns of an include list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IncludeList {
    patterns: Vec<String>,
}

/// What setting a worktree up does, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupPlan {
    /// Ignored files to copy from the main checkout, relative to its root.
    pub copy: Vec<RelPath>,
    /// Then run this in a pane of the worktree, when the Space profile has one.
    pub command: Option<CommandText>,
}

impl IncludeList {
    /// Patterns from the file's text: one per line; blank lines and `#` comments skipped. A
    /// pattern may use `*` (within a name), `?`, and `**` (across folders); a trailing `/`
    /// means everything under that folder; a pattern without `/` matches a name at any depth.
    /// A `!` line is not supported and is skipped.
    pub fn parse(text: &str) -> IncludeList {
        let patterns = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
            .map(str::to_owned)
            .collect();
        IncludeList { patterns }
    }

    /// Whether `path` (relative, `/`-separated) is listed.
    pub fn matches(&self, path: &str) -> bool {
        self.patterns.iter().any(|p| pattern_matches(p, path))
    }

    /// The setup for a worktree: of the repo's ignored files, those the list names, then the
    /// Space's setup command.
    pub fn plan(&self, ignored: &[RelPath], command: Option<CommandText>) -> SetupPlan {
        SetupPlan {
            copy: ignored
                .iter()
                .filter(|p| self.matches(p.as_str()))
                .cloned()
                .collect(),
            command: command.filter(CommandText::typable),
        }
    }
}

fn pattern_matches(pattern: &str, path: &str) -> bool {
    let anchored = pattern.trim_start_matches('/');
    let rooted = pattern.starts_with('/') || anchored.trim_end_matches('/').contains('/');
    let (glob, folder) = match anchored.strip_suffix('/') {
        Some(g) => (g, true),
        None => (anchored, false),
    };
    // Try the pattern at the root, and at every folder boundary when it is not rooted.
    let starts: Vec<usize> = match rooted {
        true => vec![0],
        false => std::iter::once(0)
            .chain(path.match_indices('/').map(|(i, _)| i + 1))
            .collect(),
    };
    starts.into_iter().any(|start| {
        let rest = &path[start..];
        match folder {
            true => rest
                .match_indices('/')
                .any(|(i, _)| glob_matches(glob, &rest[..i])),
            false => glob_matches(glob, rest) || prefix_folder(glob, rest),
        }
    })
}

/// A pattern naming a folder also names everything under it.
fn prefix_folder(glob: &str, rest: &str) -> bool {
    rest.match_indices('/')
        .any(|(i, _)| glob_matches(glob, &rest[..i]))
}

fn glob_matches(glob: &str, text: &str) -> bool {
    glob_bytes(glob.as_bytes(), text.as_bytes())
}

fn glob_bytes(glob: &[u8], text: &[u8]) -> bool {
    match glob {
        [] => text.is_empty(),
        [b'*', b'*', rest @ ..] => {
            let rest = rest.strip_prefix(b"/").unwrap_or(rest);
            (0..=text.len()).any(|i| glob_bytes(rest, &text[i..]))
        }
        [b'*', rest @ ..] => {
            let reach = text.iter().position(|b| *b == b'/').unwrap_or(text.len());
            (0..=reach).any(|i| glob_bytes(rest, &text[i..]))
        }
        [b'?', rest @ ..] => matches!(text, [c, tail @ ..] if *c != b'/' && glob_bytes(rest, tail)),
        [c, rest @ ..] => matches!(text, [t, tail @ ..] if t == c && glob_bytes(rest, tail)),
    }
}
