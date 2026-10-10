//! Reading a folder's facts (PLAN P16): git for the branch and the changes, the environment and
//! the folder for the Python virtualenv, and `.nvmrc` or `package.json` for Node. Runs on a worker
//! thread the hub starts, never on the window's. A fact that cannot be read is left out; nothing
//! here reports an error.

use std::path::Path;

use temor_core::facts::{
    EnvName, Facts, node_from_engines, node_from_nvmrc, parse_branch, parse_shortstat, venv_name,
};
use temor_core::worktree::{Git, GitCommand};

/// The most parent folders looked in for a Node version: a guard against a loop of links.
const MOST_PARENTS: usize = 64;

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

/// Everything known about `dir`. Without git (`None`), the branch and changes are left out.
pub fn read_facts(git: Option<&dyn Git>, dir: &str, env: &FactsEnv) -> Facts {
    let (branch, changes) = git.map_or((None, None), |git| read_git(git, dir));
    Facts {
        branch,
        changes,
        venv: read_venv(Path::new(dir), env),
        node: read_node(Path::new(dir)),
    }
}

fn read_git(
    git: &dyn Git,
    dir: &str,
) -> (
    Option<temor_core::facts::BranchName>,
    Option<temor_core::facts::DiffStat>,
) {
    let head = git
        .run(&git_cmd(dir, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .ok()
        .filter(|out| out.code == 0);
    // Not a repository, or one with no commit yet: there is nothing to compare either.
    let Some(head) = head else {
        return (None, None);
    };
    let changes = git
        .run(&git_cmd(dir, &["diff", "--shortstat", "HEAD"]))
        .ok()
        .filter(|out| out.code == 0)
        .and_then(|out| parse_shortstat(&out.text()));
    (parse_branch(&head.text()), changes)
}

/// The virtualenv: the environment's `VIRTUAL_ENV` if the pane has one, else a `.venv` folder
/// in the folder.
fn read_venv(dir: &Path, env: &FactsEnv) -> Option<EnvName> {
    if let Some(path) = env.virtual_env.as_deref().filter(|p| !p.is_empty()) {
        return venv_name(path);
    }
    let local = dir.join(".venv");
    match local.is_dir() {
        true => venv_name(&local.to_string_lossy()),
        false => None,
    }
}

/// The Node version from the nearest `.nvmrc` or `package.json` with `engines.node`, looking in
/// the folder and then each parent up to the repository root (the folder that holds `.git`).
fn read_node(dir: &Path) -> Option<temor_core::facts::NodeVersion> {
    for folder in dir.ancestors().take(MOST_PARENTS) {
        let from_nvmrc = std::fs::read_to_string(folder.join(".nvmrc"))
            .ok()
            .and_then(|text| node_from_nvmrc(&text));
        let found = from_nvmrc.or_else(|| {
            std::fs::read_to_string(folder.join("package.json"))
                .ok()
                .and_then(|text| node_from_engines(&text))
        });
        if found.is_some() {
            return found;
        }
        if folder.join(".git").exists() {
            return None;
        }
    }
    None
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
