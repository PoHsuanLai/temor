use super::*;
use crate::vocab::text::{CommandText, RelPath};

#[test]
fn names_come_from_tasks_and_make_safe_branches() {
    #[rustfmt::skip]
    let cases = [
        ("Fix the login bug!", Some("fix-the-login-bug")),
        ("  ../../etc/passwd ", Some("etc-passwd")),
        ("añadir tests", Some("a-adir-tests")),
        ("!!!", None),
        ("", None),
    ];
    for (task, want) in cases {
        let got = WorktreeName::from_task(task).ok();
        assert_eq!(got.as_ref().map(WorktreeName::as_str), want, "{task:?}");
    }
    let long = WorktreeName::from_task(&"word ".repeat(30)).expect("name");
    assert!(long.as_str().len() <= 40 && !long.as_str().ends_with('-'));
    assert_eq!(long.branch(), format!("agent/{}", long.as_str()));
}

#[test]
fn worktrees_live_in_the_data_dir_and_cannot_climb_out() {
    let name = WorktreeName::from_task("fix login").expect("name");
    assert_eq!(
        worktree_path("/home/p/.local/share/temor/", "My Space", &name).as_deref(),
        Ok("/home/p/.local/share/temor/worktrees/my-space/fix-login")
    );
    assert!(worktree_path("/d", "../..", &name).is_err());
}

#[test]
fn oids_are_lowercase_hex_of_a_known_length() {
    assert!(Oid::parse(&format!("{:040x}\n", 255)).is_ok());
    let bad = [
        String::new(),
        "abc".into(),
        "g".repeat(40),
        "A".repeat(40),
        "fatal: bad object".into(),
    ];
    for bad in bad {
        assert!(Oid::parse(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_choice_rows_default_to_a_new_branch() {
    assert_eq!(
        placement_rows(Placement::NewBranch),
        [Placement::NewBranch, Placement::InCheckout]
    );
    assert_eq!(
        placement_rows(Placement::InCheckout)[0],
        Placement::InCheckout
    );
    assert_eq!(MergeMethod::default(), MergeMethod::Squash);
}

#[test]
fn include_lists_pick_ignored_files_only() {
    let list = IncludeList::parse(
        "# local\n.env\n\n.env.*\nconfig/local/\n/secrets.toml\n**/*.pem\n!skipped\n",
    );
    #[rustfmt::skip]
    let cases = [
        (".env", true), ("api/.env", true), (".env.test", true),
        ("config/local/a.json", true), ("config/other/a.json", false),
        ("secrets.toml", true), ("sub/secrets.toml", false),
        ("keys/a.pem", true), ("a.pem", true),
        ("src/main.rs", false), ("skipped", false),
    ];
    for (path, want) in cases {
        assert_eq!(list.matches(path), want, "{path}");
    }
    let ignored = [RelPath(".env".into()), RelPath("target/x".into())];
    let plan = list.plan(&ignored, Some(CommandText("pnpm install".into())));
    assert_eq!(plan.copy, vec![RelPath(".env".into())]);
    assert_eq!(plan.command, Some(CommandText("pnpm install".into())));
    assert_eq!(
        list.plan(&ignored, Some(CommandText("a\nb".into())))
            .command,
        None
    );
}

#[test]
fn a_worktree_lives_through_merge_conflict_and_close() {
    use Life::{Conflicted, Discarded, Leftover, Merged, Merging, Open};
    let files = vec![RelPath("a".into())];
    let conflict = MergeOutcome::Conflicted {
        dir: "/wt".into(),
        files: files.clone(),
    };
    #[rustfmt::skip]
    let cases = vec![
        ("merge starts", Open, LifeIn::Merge(MergeMethod::Squash), Merging(MergeMethod::Squash), vec![LifeOut::RunMerge(MergeMethod::Squash)]),
        ("merge done removes it", Merging(MergeMethod::Merge), LifeIn::Finished(MergeOutcome::Merged), Merged, vec![LifeOut::Remove]),
        ("a conflict is never resolved silently", Merging(MergeMethod::Squash), LifeIn::Finished(conflict), Conflicted(files.clone()), vec![LifeOut::OpenConflictShell("/wt".into())]),
        ("a conflicted one can try again", Conflicted(files), LifeIn::Merge(MergeMethod::Merge), Merging(MergeMethod::Merge), vec![LifeOut::RunMerge(MergeMethod::Merge)]),
        ("a pull request merges nothing", Merging(MergeMethod::OpenPullRequest), LifeIn::Finished(MergeOutcome::PullRequest { branch: "agent/x".into() }), Open, vec![LifeOut::OpenPullRequest("agent/x".into())]),
        ("close and merge squashes", Open, LifeIn::CloseTab(CloseChoice::Merge), Merging(MergeMethod::Squash), vec![LifeOut::RunMerge(MergeMethod::Squash)]),
        ("close and keep leaves it listed", Open, LifeIn::CloseTab(CloseChoice::Keep), Leftover, vec![]),
        ("close and discard removes it", Open, LifeIn::CloseTab(CloseChoice::Discard), Discarded, vec![LifeOut::Remove]),
        ("a leftover can be removed", Leftover, LifeIn::DiscardLeftover, Discarded, vec![LifeOut::Remove]),
        ("a second merge while merging is ignored", Merging(MergeMethod::Squash), LifeIn::Merge(MergeMethod::Merge), Merging(MergeMethod::Squash), vec![]),
        ("nothing happens after it is gone", Discarded, LifeIn::Merge(MergeMethod::Squash), Discarded, vec![]),
    ];
    for (name, from, input, to, out) in cases {
        assert_eq!(worktree_life_step(from, input), (to, out), "{name}");
    }
}
