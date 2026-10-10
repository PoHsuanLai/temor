//! The chips' order and words.

use temor_core::facts::{BranchName, DiffStat, EnvName, Facts, NodeVersion};

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
        ("clean repository", Facts { branch: branch.clone(), changes: stat(0, 0, 0), ..Facts::default() }, vec!["main"]),
        ("everything",
            Facts { branch: branch.clone(), changes: stat(3, 10, 2), venv: venv.clone(), node: node.clone() },
            vec!["main \u{2022}", "+10 \u{2212}2", "3 files", "python tools", "node 20"]),
        ("one file", Facts { branch: branch.clone(), changes: stat(1, 1, 0), ..Facts::default() },
            vec!["main \u{2022}", "+1 \u{2212}0", "1 file"]),
        ("not a repository", Facts { venv, node, ..Facts::default() }, vec!["python tools", "node 20"]),
    ];
    for (name, facts, want) in cases {
        assert_eq!(words(&facts), want, "{name}");
    }
}
