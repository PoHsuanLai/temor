//! The hidden refs that keep a base branch's old tip before a merge, so the merge can be
//! undone: `refs/temor/undo/<n>`, numbered per repository. They are temor's own; restore points
//! of agent turns live in docket.

use super::git::{Git, GitCommand, run_ok};
use crate::error::CoreError;
use crate::vocab::ids::UndoRef;

const PREFIX: &str = "refs/temor/undo/";

/// The hidden ref for undo number `n`.
pub fn undo_ref(n: UndoRef) -> String {
    format!("{PREFIX}{}", n.0)
}

/// The undo numbers in `listing` (one ref name per line, as `for-each-ref --format=%(refname)`
/// prints them), ascending. Other refs and lines that are not numbers are ignored.
pub fn undo_ref_numbers(listing: &str) -> Vec<UndoRef> {
    let mut found: Vec<UndoRef> = listing
        .lines()
        .filter_map(|line| line.trim().strip_prefix(PREFIX)?.parse().ok().map(UndoRef))
        .collect();
    found.sort();
    found
}

/// The first number after every undo ref `repo` has, so a new merge never overwrites an old one.
pub fn next_undo(git: &dyn Git, repo: &str) -> Result<UndoRef, CoreError> {
    let cmd = GitCommand::new(repo, &["for-each-ref", "--format=%(refname)", PREFIX]);
    let found = undo_ref_numbers(&run_ok(git, &cmd)?.text());
    Ok(found.last().map_or(UndoRef(0), |n| UndoRef(n.0 + 1)))
}
