//! The chips' order and words.

use temor_core::facts::{
    BranchName, DiffStat, EnvName, Facts, NodeVersion, ToolVersion, Untracked,
};

use super::chip_texts;

fn words(facts: &Facts) -> Vec<String> {
    chip_texts(facts).into_iter().map(|c| c.text).collect()
}

#[test]
fn chips_come_in_order_and_only_for_what_is_known() {
    let branch = Some(BranchName("main".to_owned()));
    let stat = |files, added, removed| {
        Some(DiffStat {
            files,
            added,
            removed,
        })
    };
    let venv = Some(EnvName("tools".to_owned()));
    let node = Some(NodeVersion("20".to_owned()));
    #[rustfmt::skip]
    let cases: Vec<(&str, Facts, Vec<&str>)> = vec![
        ("nothing known", Facts::default(), vec![]),
        ("a branch alone is not a chip", Facts { branch: branch.clone(), changes: stat(0, 0, 0), ..Facts::default() }, vec![]),
        ("everything",
            Facts { branch: branch.clone(), changes: stat(3, 10, 2), venv: venv.clone(), node: node.clone(), untracked: Some(Untracked(4)), ..Facts::default() },
            vec!["+10 \u{2212}2", "3 files", "4 new", "python tools", "node 20"]),
        ("no untracked files adds nothing", Facts { branch: branch.clone(), untracked: Some(Untracked(0)), ..Facts::default() }, vec![]),
        ("untracked count is capped", Facts { branch: branch.clone(), untracked: Some(Untracked(1000)), ..Facts::default() }, vec!["999+ new"]),
        ("python version when there is no environment",
            Facts { python: Some(ToolVersion("3.12.1".to_owned())), ..Facts::default() }, vec!["python 3.12.1"]),
        ("the environment wins over the version",
            Facts { venv: venv.clone(), python: Some(ToolVersion("3.12.1".to_owned())), ..Facts::default() }, vec!["python tools"]),
        ("ruby and go",
            Facts { ruby: Some(ToolVersion("3.3.0".to_owned())), go: Some(ToolVersion("1.22".to_owned())), ..Facts::default() },
            vec!["ruby 3.3.0", "go 1.22"]),
        ("one file", Facts { branch: branch.clone(), changes: stat(1, 1, 0), ..Facts::default() },
            vec!["+1 \u{2212}0", "1 file"]),
        ("not a repository", Facts { venv, node, ..Facts::default() }, vec!["python tools", "node 20"]),
    ];
    for (name, facts, want) in cases {
        assert_eq!(words(&facts), want, "{name}");
    }
}
