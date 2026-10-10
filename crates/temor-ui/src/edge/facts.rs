//! Reading a folder's facts (PLAN P16): git for the branch and the changes, the environment and
//! the folder for the Python virtualenv, and the version files for Node, Python, Ruby and Go. Runs on a worker
//! thread the hub starts, never on the window's. A fact that cannot be read is left out; nothing
//! here reports an error.

use std::path::Path;

use temor_core::facts::{
    BranchName, DiffStat, Facts, Untracked, parse_branch, parse_shortstat, parse_untracked,
};
use temor_core::worktree::{Git, GitCommand};

use super::facts_files::{read_runtimes, read_venv};

/// What `read_facts` needs besides the folder.
#[derive(Debug, Clone, Default)]
pub struct FactsEnv {
    /// `VIRTUAL_ENV` from the environment the pane's shell started with, if it had one.
    pub virtual_env: Option<String>,
}

/// A git run for facts: no locks taken, so a status never blocks the person's own git.
fn git_cmd(dir: &str, args: &[&str]) -> GitCommand {
    GitCommand::new(dir, args).with_env("GIT_OPTIONAL_LOCKS", "0")
}

/// What git says about a folder.
struct GitFacts {
    branch: Option<BranchName>,
    changes: Option<DiffStat>,
    untracked: Option<Untracked>,
}

/// Everything known about `dir`. Without git (`None`), the branch, changes and untracked count
/// are left out.
pub fn read_facts(git: Option<&dyn Git>, dir: &str, env: &FactsEnv) -> Facts {
    let git = git.map(|git| read_git(git, dir));
    let (branch, changes, untracked) =
        git.map_or((None, None, None), |g| (g.branch, g.changes, g.untracked));
    let runtimes = read_runtimes(Path::new(dir));
    Facts {
        branch,
        changes,
        untracked,
        venv: read_venv(Path::new(dir), env),
        node: runtimes.node,
        python: runtimes.python,
        ruby: runtimes.ruby,
        go: runtimes.go,
    }
}

/// The output of a git run that succeeded.
fn ok_text(git: &dyn Git, dir: &str, args: &[&str]) -> Option<String> {
    git.run(&git_cmd(dir, args))
        .ok()
        .filter(|out| out.code == 0)
        .map(|out| out.text())
}

fn read_git(git: &dyn Git, dir: &str) -> GitFacts {
    let nothing = GitFacts {
        branch: None,
        changes: None,
        untracked: None,
    };
    let head = ok_text(git, dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let branch = match head {
        Some(text) => parse_branch(&text),
        // A repository with no commit has no `HEAD` to resolve, but it has a branch name.
        None => ok_text(git, dir, &["symbolic-ref", "--short", "HEAD"])
            .and_then(|text| parse_branch(&text)),
    };
    // Not a repository: there is nothing to compare or count either.
    let Some(branch) = branch else {
        return nothing;
    };
    GitFacts {
        branch: Some(branch),
        // Without a commit `diff HEAD` fails, so no changes are shown.
        changes: ok_text(git, dir, &["diff", "--shortstat", "HEAD"])
            .and_then(|text| parse_shortstat(&text)),
        untracked: ok_text(git, dir, &["status", "--porcelain=v1", "-uall"])
            .map(|text| parse_untracked(&text)),
    }
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
