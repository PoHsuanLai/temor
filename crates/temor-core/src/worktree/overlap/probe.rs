//! The git questions of the overlap check, as the spike validated them: `merge-tree
//! --write-tree --name-only -z` for conflicts, and a same-file intersection of `diff --no-renames
//! --name-only -z` on both sides of the merge base (this catches a rename against an edit).
//! Parsing is pure; `probe_pair` and `probe_base` run git through the injected [`Git`].

use crate::error::CoreError;
use crate::vocab::text::RelPath;
use crate::worktree::git::{Git, GitCommand, GitOutput, run_ok};
use crate::worktree::model::Oid;

/// What comparing two snapshots found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PairProbe {
    /// Files a merge would conflict in.
    pub conflicts: Vec<RelPath>,
    /// Files both changed since their merge base.
    pub same_file: Vec<RelPath>,
}

/// What comparing an agent's snapshot with the base tip found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BaseProbe {
    /// Files a merge into the base would conflict in.
    pub conflicts: Vec<RelPath>,
    /// Files the base changed since the merge base.
    pub base_files: Vec<RelPath>,
    /// Files the agent changed since the merge base.
    pub agent_files: Vec<RelPath>,
    /// Commits only the agent has.
    pub ahead: u32,
    /// Commits only the base has.
    pub behind: u32,
}

/// The conflicted paths in a `merge-tree --write-tree --name-only -z` run. Exit 0 is clean and
/// exit 1 is a conflict, but exit 1 is also what a bad ref gives, so the first field must be a
/// tree id either way.
pub fn parse_merge_tree(out: &GitOutput) -> Result<Vec<RelPath>, CoreError> {
    let bad = || CoreError::GitOutput {
        what: "merge-tree".into(),
    };
    if !matches!(out.code, 0 | 1) {
        return Err(CoreError::git_failed("merge-tree", &out.stderr));
    }
    let mut fields = out.stdout.split(|b| *b == 0);
    let first = fields.next().ok_or_else(bad)?;
    Oid::parse(&String::from_utf8_lossy(first)).map_err(|_| bad())?;
    let mut paths: Vec<RelPath> = fields.take_while(|f| !f.is_empty()).map(path).collect();
    paths.sort();
    paths.dedup();
    match (out.code, paths.is_empty()) {
        (0, _) | (1, false) => Ok(paths),
        _ => Err(bad()),
    }
}

/// Paths from a `-z` name list.
pub fn parse_names(stdout: &[u8]) -> Vec<RelPath> {
    let mut paths: Vec<RelPath> = stdout
        .split(|b| *b == 0)
        .filter(|f| !f.is_empty())
        .map(path)
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

fn path(bytes: &[u8]) -> RelPath {
    RelPath(String::from_utf8_lossy(bytes).into_owned())
}

/// The paths in both sorted lists.
pub fn intersect(a: &[RelPath], b: &[RelPath]) -> Vec<RelPath> {
    a.iter()
        .filter(|p| b.binary_search(p).is_ok())
        .cloned()
        .collect()
}

/// `(left, right)` from `rev-list --left-right --count`.
pub fn parse_counts(text: &str) -> Result<(u32, u32), CoreError> {
    let mut parts = text.split_whitespace().map(str::parse::<u32>);
    match (parts.next(), parts.next(), parts.next()) {
        (Some(Ok(left)), Some(Ok(right)), None) => Ok((left, right)),
        _ => Err(CoreError::GitOutput {
            what: "rev-list".into(),
        }),
    }
}

fn conflicts_between(
    git: &dyn Git,
    repo: &str,
    a: &Oid,
    b: &Oid,
) -> Result<Vec<RelPath>, CoreError> {
    let cmd = GitCommand::new(
        repo,
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "-z",
            a.as_str(),
            b.as_str(),
        ],
    );
    parse_merge_tree(&git.run(&cmd)?)
}

/// The commit both descend from, or none for unrelated histories.
fn merge_base(git: &dyn Git, repo: &str, a: &Oid, b: &Oid) -> Result<Option<Oid>, CoreError> {
    let cmd = GitCommand::new(repo, &["merge-base", a.as_str(), b.as_str()]);
    let out = git.run(&cmd)?;
    match out.code {
        0 => Ok(Some(Oid::parse(&out.text())?)),
        1 => Ok(None),
        _ => Err(CoreError::git_failed(cmd.what(), &out.stderr)),
    }
}

fn changed_since(
    git: &dyn Git,
    repo: &str,
    from: &Oid,
    to: &Oid,
) -> Result<Vec<RelPath>, CoreError> {
    let cmd = GitCommand::new(
        repo,
        &[
            "diff",
            "--no-renames",
            "--name-only",
            "-z",
            from.as_str(),
            to.as_str(),
        ],
    );
    Ok(parse_names(&run_ok(git, &cmd)?.stdout))
}

/// Compares two snapshot (or tip) commits. Both must already be known commits.
pub fn probe_pair(git: &dyn Git, repo: &str, a: &Oid, b: &Oid) -> Result<PairProbe, CoreError> {
    let conflicts = conflicts_between(git, repo, a, b)?;
    let same_file = match merge_base(git, repo, a, b)? {
        Some(base) => intersect(
            &changed_since(git, repo, &base, a)?,
            &changed_since(git, repo, &base, b)?,
        ),
        None => Vec::new(),
    };
    Ok(PairProbe {
        conflicts,
        same_file,
    })
}

/// Compares an agent snapshot with the base tip.
pub fn probe_base(
    git: &dyn Git,
    repo: &str,
    base: &Oid,
    agent: &Oid,
) -> Result<BaseProbe, CoreError> {
    let conflicts = conflicts_between(git, repo, agent, base)?;
    let (base_files, agent_files) = match merge_base(git, repo, agent, base)? {
        Some(mb) => (
            changed_since(git, repo, &mb, base)?,
            changed_since(git, repo, &mb, agent)?,
        ),
        None => (Vec::new(), Vec::new()),
    };
    let range = format!("{}...{}", base.as_str(), agent.as_str());
    let counts = run_ok(
        git,
        &GitCommand::new(
            repo,
            &["rev-list", "--left-right", "--count", range.as_str()],
        ),
    )?;
    let (behind, ahead) = parse_counts(&counts.text())?;
    Ok(BaseProbe {
        conflicts,
        base_files,
        agent_files,
        ahead,
        behind,
    })
}
