//! What the agent jobs do to real repositories (scratch ones, made with the real git; skipped when
//! git is missing unless `TEMOR_REQUIRE_TOOLS` is set): the repository a folder is in, a worktree made
//! from project facts, and a merge back.

use std::fs;
use std::path::Path;

use temor_core::worktree::{GitCommand, MergeOutcome, resolve_commit, run_ok};
use temor_ui::hub::agents::{
    MakeRequest, discard, find_leftovers, listed_as_worktree, make_worktree, merge_back,
    snapshot_tree, toplevel, unused_worktrees,
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
fn the_repository_of_a_folder_is_found_from_inside_it_and_outside_there_is_none() {
    let Some(s) = scratch() else { return };
    let real = path(&fs::canonicalize(&s.repo).expect("real path"));
    assert_eq!(toplevel(&s.git, &s.repo).as_deref(), Some(real.as_str()));

    let inside = Path::new(&s.repo).join("deep/er");
    fs::create_dir_all(&inside).expect("folders");
    assert_eq!(
        toplevel(&s.git, &path(&inside)).as_deref(),
        Some(real.as_str())
    );

    let outside = tempfile::tempdir().expect("outside");
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
    // Recorded as its real path, as git lists it.
    let data = path(&fs::canonicalize(&s.data).expect("real data path"));
    assert!(
        one.worktree.path.starts_with(&data),
        "{}",
        one.worktree.path
    );
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
        &GitCommand::new(&s.repo, &["rev-parse", "--verify", "refs/temor/undo/0"]),
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

/// Why: git lists a worktree's real path while a tab's shell reports the path it was reached by,
/// so under a symlinked data folder (Fedora Atomic's `/home` is `/var/home`) a worktree a tab
/// was working in was offered as a leftover, and discarding it would have removed that work.
#[test]
fn a_worktree_a_tab_reached_through_a_symlink_is_not_a_leftover() {
    let Some(s) = scratch() else { return };
    let link = tempfile::tempdir().expect("link dir");
    let data = link.path().join("data");
    std::os::unix::fs::symlink(&s.data, &data).expect("symlinked data folder");
    let made = make_worktree(
        &s.git,
        &MakeRequest {
            data_dir: path(&data),
            ..request(&s)
        },
    )
    .expect("worktree");
    let listed = find_leftovers(&s.git, &s.repo, &data).expect("list");
    assert_eq!(listed.len(), 1, "{listed:?}");
    // The worktree is recorded as git lists it.
    assert_eq!(listed[0].path, made.worktree.path);

    let name = Path::new(&made.worktree.path).file_name().expect("name");
    let reached = data.join("worktrees/my-space").join(name).join("src");
    let free = unused_worktrees(listed.clone(), &[path(&reached)]);
    assert!(free.is_empty(), "a tab works in it: {free:?}");
    assert_eq!(unused_worktrees(listed, &[]).len(), 1);
}
