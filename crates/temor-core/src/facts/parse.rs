//! Reading what git and the environment say. Anything unreadable is `None`.

use super::model::{BranchName, DiffStat, EnvName, MOST_UNTRACKED, Untracked};

/// The branch from `git rev-parse --abbrev-ref HEAD`. A detached `HEAD` has no branch name.
pub fn parse_branch(output: &str) -> Option<BranchName> {
    let name = output.lines().next()?.trim();
    match name {
        "" | "HEAD" => None,
        name => Some(BranchName(name.to_owned())),
    }
}

/// The counts from `git diff --shortstat HEAD`, such as
/// ` 3 files changed, 10 insertions(+), 2 deletions(-)`. Empty output means no changes. Either
/// of the two line counts may be missing (git leaves out a zero).
pub fn parse_shortstat(output: &str) -> Option<DiffStat> {
    let line = output.lines().find(|l| !l.trim().is_empty());
    let Some(line) = line else {
        return Some(DiffStat::default());
    };
    let mut stat = DiffStat::default();
    let mut seen_files = false;
    for part in line.split(',') {
        let mut words = part.split_whitespace();
        let Some(count) = words.next().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        match words.next() {
            Some(w) if w.starts_with("file") => {
                stat.files = count;
                seen_files = true;
            }
            Some(w) if w.starts_with("insertion") => stat.added = count,
            Some(w) if w.starts_with("deletion") => stat.removed = count,
            _ => {}
        }
    }
    seen_files.then_some(stat)
}

/// The virtualenv's name: the last part of `VIRTUAL_ENV`. When that folder is just `.venv`, the
/// project folder above it names the environment, as Python's prompt does.
pub fn venv_name(path: &str) -> Option<EnvName> {
    let mut parts = path.rsplit('/').filter(|p| !p.is_empty());
    let last = parts.next()?;
    let name = match last {
        ".venv" | "venv" => parts.next().unwrap_or(last),
        other => other,
    };
    Some(EnvName(name.to_owned()))
}

/// How many files `git status --porcelain=v1 -uall` lists as untracked (lines starting `?? `),
/// stopping past [`MOST_UNTRACKED`] so a huge folder stays cheap to show.
pub fn parse_untracked(output: &str) -> Untracked {
    let count = output
        .lines()
        .filter(|l| l.starts_with("?? "))
        .take(MOST_UNTRACKED as usize + 1)
        .count();
    Untracked(u32::try_from(count).unwrap_or(MOST_UNTRACKED + 1))
}
