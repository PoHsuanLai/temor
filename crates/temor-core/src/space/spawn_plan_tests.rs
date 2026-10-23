//! The base layer under carried state: who wins, and what a move between Spaces swaps.

use super::{Origin, SpawnPlan, spawn_plan};
use crate::session::Replay;

fn vars(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn carried(env: &[(&str, &str)]) -> Replay {
    Replay {
        cwd: Some("/work".into()),
        env: vars(env),
        aliases: vars(&[("g", "git")]),
        withheld: vec!["GITHUB_TOKEN".into()],
    }
}

fn names(plan: &SpawnPlan) -> Vec<(&str, &str, Origin)> {
    plan.env
        .iter()
        .map(|(k, v, o)| (k.as_str(), v.as_str(), *o))
        .collect()
}

#[test]
fn nothing_carried_is_the_spaces_own_start() {
    let plan = spawn_plan(&[], &vars(&[("B", "2"), ("A", "1")]), Some("/p"), None);
    assert_eq!(
        names(&plan),
        [("A", "1", Origin::Space), ("B", "2", Origin::Space)]
    );
    assert_eq!(plan.cwd.as_deref(), Some("/p"));
}

#[test]
fn what_the_person_set_lies_over_the_space_and_what_the_space_set_follows_it() {
    let old = vars(&[("CLUSTER", "dev"), ("REGION", "eu")]);
    let new = vars(&[("CLUSTER", "prod"), ("TEAM", "core")]);
    // The old shell kept CLUSTER as the old Space set it, changed REGION, and added EDITOR.
    let state = carried(&[("CLUSTER", "dev"), ("REGION", "us"), ("EDITOR", "vim")]);
    let plan = spawn_plan(&old, &new, Some("/p"), Some(&state));
    assert_eq!(
        names(&plan),
        [
            ("CLUSTER", "prod", Origin::Space),
            ("EDITOR", "vim", Origin::User),
            ("REGION", "us", Origin::User),
            ("TEAM", "core", Origin::Space),
        ]
    );
}

#[test]
fn a_variable_the_old_space_set_and_the_new_one_does_not_is_dropped() {
    let plan = spawn_plan(
        &vars(&[("OLD_ONLY", "x")]),
        &vars(&[]),
        None,
        Some(&carried(&[("OLD_ONLY", "x")])),
    );
    assert_eq!(names(&plan), []);
}

#[test]
fn the_carried_folder_wins_and_secrets_stay_withheld() {
    let plan = spawn_plan(&[], &[], Some("/p"), Some(&carried(&[])));
    assert_eq!(plan.cwd.as_deref(), Some("/work"));
    assert_eq!(plan.aliases, vars(&[("g", "git")]));
    assert_eq!(plan.withheld, ["GITHUB_TOKEN"]);
    let none = Replay {
        cwd: None,
        ..carried(&[])
    };
    assert_eq!(
        spawn_plan(&[], &[], Some("/p"), Some(&none)).cwd.as_deref(),
        Some("/p"),
        "no carried folder: the Space's"
    );
}
