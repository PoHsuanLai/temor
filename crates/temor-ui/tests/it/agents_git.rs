//! What the agent jobs do to real repositories (scratch ones, made with the real git; skipped when
//! git is missing unless `TEMOR_REQUIRE_TOOLS` is set): checkpoint numbering, a worktree made from
//! project facts, files put back only inside the repository, and a merge back.

use std::fs;
use std::path::Path;

use temor_core::checkpoint::{restore_plan, take};
use temor_core::vocab::ids::CheckpointRef;
use temor_core::vocab::text::RelPath;
use temor_core::worktree::{GitCommand, MergeOutcome, resolve_commit, run_ok};
use temor_ui::hub::agents::{
    MakeRequest, discard, find_leftovers, listed_as_worktree, make_worktree, merge_back, restore,
    seed, snapshot_tree, toplevel,
};

use super::worktree_git::{Scratch, path, scratch};

fn request(s: &Scratch) -> MakeRequest {
    MakeRequest {
        cwd: s.repo.clone(),
        data_dir: s.data.clone(),
        space: "My Space".to_owned(),
    }
}

fn commit_all(s: &Scratch, message: &str) {
    let run = |args: &[&str]| run_ok(&s.git, &GitCommand::new(&s.repo, args)).expect("git");
    run(&["add", "-A"]);
    run(&["commit", "-q", "-m", message]);
}

#[test]
fn a_run_is_numbered_after_the_checkpoints_the_repository_has() {
    let Some(s) = scratch() else { return };
    let real = path(&fs::canonicalize(&s.repo).expect("real path"));
    let first = seed(&s.git, &s.repo);
    assert_eq!(
        (first.root.as_deref(), first.next),
        (Some(real.as_str()), CheckpointRef(0))
    );
    take(&s.git, &s.repo, CheckpointRef(0)).expect("take 0");
    take(&s.git, &s.repo, CheckpointRef(1)).expect("take 1");

    // From a folder inside the repository the answer is the same repository.
    let inside = Path::new(&s.repo).join("deep/er");
    fs::create_dir_all(&inside).expect("folders");
    let again = seed(&s.git, &path(&inside));
    assert_eq!(
        (again.root.as_deref(), again.next),
        (Some(real.as_str()), CheckpointRef(2))
    );

    // A folder outside every repository has none, and numbers from zero.
    let outside = tempfile::tempdir().expect("outside");
    let none = seed(&s.git, &path(outside.path()));
    assert_eq!((none.root, none.next), (None, CheckpointRef(0)));
    assert_eq!(toplevel(&s.git, &path(outside.path())), None);
}

#[test]
fn a_worktree_gets_the_ignored_files_the_project_lists_and_nothing_else() {
    let Some(s) = scratch() else { return };
    let root = Path::new(&s.repo);
    fs::write(root.join(".gitignore"), ".env\nlocal/\nbuild.log\n").expect("ignore");
    fs::write(
        root.join(".worktreeinclude"),
        "# what a worktree needs\n.env\nlocal/*.toml\n",
    )
    .expect("include");
    commit_all(&s, "project facts");
    fs::write(root.join(".env"), "TOKEN=1\n").expect(".env");
    fs::create_dir_all(root.join("local")).expect("local");
    fs::write(root.join("local/config.toml"), "a = 1\n").expect("config");
    fs::write(root.join("local/cache.bin"), "x").expect("cache");
    fs::write(root.join("build.log"), "noise\n").expect("log");

    let one = make_worktree(&s.git, &request(&s)).expect("first worktree");
    let wt = Path::new(&one.worktree.path);
    assert_eq!(one.copied, 2);
    assert_eq!(
        fs::read_to_string(wt.join(".env")).expect(".env"),
        "TOKEN=1\n"
    );
    assert!(wt.join("local/config.toml").exists());
    assert!(!wt.join("local/cache.bin").exists() && !wt.join("build.log").exists());
    assert!(one.worktree.path.starts_with(&s.data));
    assert!(one.worktree.path.contains("/worktrees/my-space/"));

    // The next one takes the next free name, from the same repository.
    let two = make_worktree(&s.git, &request(&s)).expect("second worktree");
    assert!(one.worktree.branch().ends_with("-1") && two.worktree.branch().ends_with("-2"));

    // Both are listed as leftovers while no tab uses them; discarding one removes it and its branch.
    let listed = find_leftovers(&s.git, &s.repo, Path::new(&s.data)).expect("list");
    assert_eq!(listed.len(), 2, "{listed:?}");
    let gone = listed
        .iter()
        .find(|w| w.path == one.worktree.path)
        .expect("the first is listed");
    let target = listed_as_worktree(&s.repo, gone).expect("as a worktree");
    discard(&s.git, &target).expect("discard");
    assert!(!wt.exists());
    let left = find_leftovers(&s.git, &s.repo, Path::new(&s.data)).expect("list again");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].path, two.worktree.path);
}

#[test]
fn putting_files_back_removes_what_was_made_since_and_stays_inside_the_repository() {
    let Some(s) = scratch() else { return };
    let root = Path::new(&s.repo);
    take(&s.git, &s.repo, CheckpointRef(0)).expect("take");
    fs::write(root.join("made.txt"), "new\n").expect("made");
    fs::create_dir_all(root.join("adir")).expect("adir");
    fs::write(root.join("a.txt"), "agent was here\n").expect("a");

    // Someone else's file elsewhere, and a plan that tries to reach it.
    let elsewhere = Path::new(&s.data).join("keep.txt");
    fs::write(&elsewhere, "mine\n").expect("keep");
    let data_name = Path::new(&s.data)
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    let mut plan = restore_plan(&s.git, &s.repo, CheckpointRef(0)).expect("plan");
    assert!(plan.delete.contains(&RelPath("made.txt".into())));
    for sneaky in [
        format!("../{data_name}/keep.txt"),
        "/etc/hostname".to_owned(),
        "adir".to_owned(),
    ] {
        plan.delete.push(RelPath(sneaky));
    }

    let removed = restore(&s.git, &s.repo, CheckpointRef(0), &plan).expect("restore");
    assert_eq!(removed, 1, "only the file made since");
    assert!(!root.join("made.txt").exists());
    assert!(root.join("adir").is_dir(), "a folder is never removed");
    assert!(
        elsewhere.exists(),
        "nothing outside the repository is touched"
    );
    assert_ne!(
        fs::read_to_string(root.join("a.txt")).expect("a"),
        "agent was here\n"
    );
}

#[test]
fn work_left_uncommitted_is_merged_back_and_the_old_tip_is_kept() {
    let Some(s) = scratch() else { return };
    let made = make_worktree(&s.git, &request(&s)).expect("worktree");
    let wt = made.worktree;
    fs::write(Path::new(&wt.path).join("feature.txt"), "done\n").expect("feature");

    // The overlap check sees uncommitted work, against the base's tip.
    let (tip, snap) = snapshot_tree(&s.git, &wt).expect("snapshot");
    assert_eq!(tip, wt.base);
    let files = run_ok(
        &s.git,
        &GitCommand::new(
            &wt.path,
            &["ls-tree", "-r", "--name-only", snap.commit.as_str()],
        ),
    )
    .expect("ls-tree")
    .text();
    assert!(files.lines().any(|f| f == "feature.txt"), "{files}");

    let outcome = merge_back(&s.git, &wt).expect("merge");
    assert_eq!(outcome, MergeOutcome::Merged);
    assert_eq!(
        fs::read_to_string(Path::new(&s.repo).join("feature.txt")).expect("merged"),
        "done\n"
    );
    let log = run_ok(&s.git, &GitCommand::new(&s.repo, &["log", "--format=%s"]))
        .expect("log")
        .text();
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        [format!("Merge {}", wt.branch()), "base".to_owned()]
    );
    run_ok(
        &s.git,
        &GitCommand::new(
            &s.repo,
            &["rev-parse", "--verify", "refs/temor/checkpoints/0"],
        ),
    )
    .expect("the old tip is kept for undo");
    assert_ne!(
        resolve_commit(&s.git, &s.repo, "HEAD").expect("head"),
        wt.base
    );

    discard(&s.git, &wt).expect("discard");
    assert!(!Path::new(&wt.path).exists());
    let branches = run_ok(
        &s.git,
        &GitCommand::new(&s.repo, &["branch", "--list", "agent/*"]),
    )
    .expect("branches");
    assert!(branches.stdout.is_empty());
}
