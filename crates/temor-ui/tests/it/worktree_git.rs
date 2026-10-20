//! Agent worktrees and the refs temor keeps on scratch repositories made with the real git: a private
//! HOME-free setup (`GIT_CONFIG_GLOBAL=/dev/null`, a fixed identity), nothing outside temp dirs.
//! Skips when git is missing, unless `TEMOR_REQUIRE_TOOLS` is set.

use std::fs;
use std::path::Path;

use temor_core::vocab::ids::{TabId, UndoRef};
use temor_core::vocab::text::RelPath;
use temor_core::worktree::overlap::{probe_base, probe_pair};
use temor_core::worktree::{
    AgentWorktree, CreateParams, Git, GitCommand, MergeMethod, MergeOutcome, WorktreeName, create,
    merge, next_undo, remove, resolve_commit, run_ok, snapshot, sweep_old_snapshots,
};
use temor_ui::edge::git::ProcessGit;

pub(super) struct Scratch {
    pub(super) git: ProcessGit,
    pub(super) repo: String,
    pub(super) data: String,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

fn git_runner() -> Option<ProcessGit> {
    let git = ProcessGit::new("git")
        .with_env("GIT_CONFIG_GLOBAL", "/dev/null")
        .with_env("GIT_CONFIG_NOSYSTEM", "1")
        .with_env("GIT_AUTHOR_NAME", "Test")
        .with_env("GIT_AUTHOR_EMAIL", "test@example.com")
        .with_env("GIT_COMMITTER_NAME", "Test")
        .with_env("GIT_COMMITTER_EMAIL", "test@example.com");
    match git.run(&GitCommand::new(".", &["--version"])) {
        Ok(out) if out.code == 0 => Some(git),
        _ => {
            temor_testkit::skip("git", "worktree_git");
            None
        }
    }
}

pub(super) fn path(p: &Path) -> String {
    p.to_str().expect("utf-8 temp path").to_owned()
}

pub(super) fn lines(prefix: &str, n: usize) -> String {
    (1..=n).map(|i| format!("{prefix}{i}\n")).collect()
}

pub(super) fn scratch() -> Option<Scratch> {
    let git = git_runner()?;
    let repo_dir = tempfile::tempdir().expect("repo dir");
    let data_dir = tempfile::tempdir().expect("data dir");
    let repo = path(repo_dir.path());
    let run = |args: &[&str]| run_ok(&git, &GitCommand::new(&repo, args)).expect("git");
    run(&["init", "-q", "-b", "main"]);
    fs::write(repo_dir.path().join("a.txt"), lines("line ", 20)).expect("a");
    fs::write(repo_dir.path().join("c.txt"), lines("c ", 5)).expect("c");
    run(&["add", "-A"]);
    run(&["commit", "-q", "-m", "base"]);
    Some(Scratch {
        git,
        repo,
        data: path(data_dir.path()),
        _dirs: (repo_dir, data_dir),
    })
}

fn agent(s: &Scratch, tab: u64, task: &str) -> AgentWorktree {
    create(
        &s.git,
        &CreateParams {
            tab: TabId(tab),
            repo: s.repo.clone(),
            data_dir: s.data.clone(),
            space: "scratch space".into(),
            name: WorktreeName::from_task(task).expect("name"),
            base_ref: "main".into(),
        },
    )
    .expect("create")
}

fn edit(wt: &AgentWorktree, file: &str, body: &str) {
    fs::write(Path::new(&wt.path).join(file), body).expect("edit");
}

fn rel(p: &str) -> RelPath {
    RelPath(p.into())
}

#[test]
fn creating_a_worktree_records_the_base_and_lives_in_the_data_dir() {
    let Some(s) = scratch() else { return };
    let wt = agent(&s, 1, "Fix login");
    assert!(wt.path.starts_with(&s.data) && wt.path.ends_with("worktrees/scratch-space/fix-login"));
    assert_eq!(wt.branch(), "agent/fix-login");
    assert_eq!(
        wt.base,
        resolve_commit(&s.git, &s.repo, "main").expect("main")
    );
    assert!(resolve_commit(&s.git, &s.repo, "no-such-ref").is_err());
    remove(&s.git, &wt).expect("remove");
    assert!(!Path::new(&wt.path).exists());
}

#[test]
fn snapshots_and_probes_find_what_the_spike_found() {
    let Some(s) = scratch() else { return };
    let (one, two) = (agent(&s, 1, "one"), agent(&s, 2, "two"));
    let a = lines("line ", 20);

    // Same line edited differently, uncommitted on both sides: a conflict.
    edit(&one, "a.txt", &a.replacen("line 2\n", "ONE\n", 1));
    edit(&two, "a.txt", &a.replacen("line 2\n", "TWO\n", 1));
    // A rename on one side against an edit on the other: merges clean, but the same file.
    fs::rename(
        Path::new(&one.path).join("c.txt"),
        Path::new(&one.path).join("c2.txt"),
    )
    .expect("mv");
    edit(&two, "c.txt", &lines("c ", 4));
    // The same new path on both sides, untracked: add/add.
    edit(&one, "new.txt", "from one\n");
    edit(&two, "new.txt", "from two\n");

    let (s1, s2) = (
        snapshot(&s.git, &one.path).expect("s1"),
        snapshot(&s.git, &two.path).expect("s2"),
    );
    let probe = probe_pair(&s.git, &s.repo, &s1.commit, &s2.commit).expect("pair");
    assert_eq!(probe.conflicts, vec![rel("a.txt"), rel("new.txt")]);
    assert_eq!(
        probe.same_file,
        vec![rel("a.txt"), rel("c.txt"), rel("new.txt")]
    );

    // The agents' own index and files were not touched: nothing is staged.
    let staged = run_ok(
        &s.git,
        &GitCommand::new(&one.path, &["diff", "--cached", "--name-only"]),
    )
    .expect("diff");
    assert!(staged.stdout.is_empty());
    assert!(Path::new(&one.path).join("new.txt").exists());

    // Against the base, which has moved on a different line of a.txt.
    run_ok(
        &s.git,
        &GitCommand::new(&s.repo, &["checkout", "-q", "main"]),
    )
    .expect("main");
    fs::write(
        Path::new(&s.repo).join("a.txt"),
        a.replacen("line 20\n", "BASE\n", 1),
    )
    .expect("base edit");
    run_ok(
        &s.git,
        &GitCommand::new(&s.repo, &["commit", "-q", "-am", "base moves"]),
    )
    .expect("commit");
    let tip = resolve_commit(&s.git, &s.repo, "main").expect("tip");
    let base = probe_base(&s.git, &s.repo, &tip, &s1.commit).expect("base");
    assert_eq!(base.conflicts, Vec::<RelPath>::new());
    assert_eq!(
        (base.base_files.clone(), base.behind, base.ahead),
        (vec![rel("a.txt")], 1, 1)
    );
    assert!(base.agent_files.contains(&rel("a.txt")));
}

#[test]
fn squash_merges_and_a_conflict_is_reported_not_resolved() {
    let Some(s) = scratch() else { return };
    let (one, two) = (agent(&s, 1, "one"), agent(&s, 2, "two"));
    let a = lines("line ", 20);
    for (wt, word) in [(&one, "ONE"), (&two, "TWO")] {
        edit(
            wt,
            "a.txt",
            &a.replacen("line 2\n", &format!("{word}\n"), 1),
        );
        run_ok(
            &s.git,
            &GitCommand::new(&wt.path, &["commit", "-q", "-am", word]),
        )
        .expect("commit");
    }
    let merged = merge(&s.git, &one, MergeMethod::Squash, "Squash one", UndoRef(0)).expect("merge");
    assert_eq!(merged, MergeOutcome::Merged);
    let log = run_ok(&s.git, &GitCommand::new(&s.repo, &["log", "--format=%s"]))
        .expect("log")
        .text();
    assert_eq!(log.lines().collect::<Vec<_>>(), ["Squash one", "base"]);
    let kept = run_ok(
        &s.git,
        &GitCommand::new(&s.repo, &["rev-parse", "--verify", "refs/temor/undo/0"]),
    );
    assert!(kept.is_ok(), "the old tip is kept for undo");

    let stuck = merge(&s.git, &two, MergeMethod::Squash, "Squash two", UndoRef(1)).expect("merge");
    assert_eq!(
        stuck,
        MergeOutcome::Conflicted {
            dir: s.repo.clone(),
            files: vec![rel("a.txt")]
        }
    );
}

#[test]
fn old_snapshot_refs_are_swept_and_merge_undo_numbers_go_on_after_the_highest() {
    let Some(s) = scratch() else { return };
    let run = |args: &[&str]| run_ok(&s.git, &GitCommand::new(&s.repo, args)).expect("git");
    // A snapshot commit dated long ago, one made now, and an undo ref.
    let old = run_ok(
        &s.git,
        &GitCommand::new(&s.repo, &["commit-tree", "HEAD^{tree}", "-m", "old"])
            .with_env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z"),
    )
    .expect("old commit")
    .text();
    run(&["update-ref", "refs/temor/checkpoints/3", old.trim()]);
    run(&["update-ref", "refs/temor/checkpoints/4", "HEAD"]);
    run(&["update-ref", "refs/temor/undo/6", "HEAD"]);

    let now = temor_ui::edge::unix_now().0;
    assert_eq!(sweep_old_snapshots(&s.git, &s.repo, now).expect("sweep"), 1);
    let left = run(&["for-each-ref", "--format=%(refname)", "refs/temor/"]).text();
    assert_eq!(
        left.lines().collect::<Vec<_>>(),
        ["refs/temor/checkpoints/4", "refs/temor/undo/6"]
    );
    assert_eq!(sweep_old_snapshots(&s.git, &s.repo, now).expect("again"), 0);
    assert_eq!(next_undo(&s.git, &s.repo).expect("next"), UndoRef(7));
}
