//! The chips' order and words, and when they stand over the prompt's header line.

use temor_core::facts::{
    BranchName, DiffStat, EnvName, Facts, NodeVersion, ToolVersion, Untracked,
};

use super::{ChipIcon, Tone, chip_texts, chips_go_over, lead_chips};
use crate::session::{HeaderLine, PromptHeader};

fn words(facts: &Facts) -> Vec<String> {
    chip_texts(facts).iter().map(|c| c.text()).collect()
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
            vec!["+10 \u{2212}2 \u{b7} 3 files", "4 new", "python tools", "node 20"]),
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
            vec!["+1 \u{2212}0 \u{b7} 1 file"]),
        ("not a repository", Facts { venv, node, ..Facts::default() }, vec!["python tools", "node 20"]),
    ];
    for (name, facts, want) in cases {
        assert_eq!(words(&facts), want, "{name}");
    }
}

#[test]
fn the_changes_chip_colours_its_counts() {
    let facts = Facts {
        changes: Some(DiffStat {
            files: 2,
            added: 5,
            removed: 1,
        }),
        ..Facts::default()
    };
    let chips = chip_texts(&facts);
    assert_eq!(chips[0].icon, Some(ChipIcon::Changes));
    let tones: Vec<(&str, Tone)> = chips[0]
        .parts
        .iter()
        .map(|p| (p.text.as_str(), p.tone))
        .collect();
    assert_eq!(
        tones,
        [
            ("+5", Tone::Added),
            ("\u{2212}1", Tone::Removed),
            ("\u{b7}", Tone::Plain),
            ("2 files", Tone::Plain)
        ]
    );
}

#[test]
fn the_folder_and_branch_lead_and_the_branch_only_when_known() {
    let branch = Facts {
        branch: Some(BranchName("main".to_owned())),
        ..Facts::default()
    };
    let lead = |facts: &Facts| -> Vec<(Option<ChipIcon>, String)> {
        lead_chips("~/temor", facts)
            .iter()
            .map(|c| (c.icon, c.text()))
            .collect()
    };
    assert_eq!(
        lead(&branch),
        [
            (Some(ChipIcon::Folder), "~/temor".to_owned()),
            (Some(ChipIcon::Branch), "main".to_owned())
        ]
    );
    assert_eq!(
        lead(&Facts::default()),
        [(Some(ChipIcon::Folder), "~/temor".to_owned())]
    );
}

#[test]
fn the_chips_cover_only_a_header_line_that_says_the_folder() {
    let header = |line, text: &str| PromptHeader {
        row: 3,
        line,
        text: text.to_owned(),
    };
    #[rustfmt::skip]
    let cases = [
        ("temor's prompt", header(HeaderLine::Own, "~/temor  main"), Some("~/temor"), true),
        ("temor's prompt outside a repository", header(HeaderLine::Own, "~/temor"), Some("~/temor"), true),
        ("a one-line prompt", header(HeaderLine::Shared, "~/temor $"), Some("~/temor"), false),
        ("the person's own two-line prompt", header(HeaderLine::Own, "pohsuan@box ~/temor"), Some("~/temor"), false),
        ("a longer folder with the same start", header(HeaderLine::Own, "~/temor-lanes"), Some("~/temor"), false),
        ("another computer", header(HeaderLine::Own, "~/temor  main"), None, false),
        ("an empty folder", header(HeaderLine::Own, "~/temor"), Some(""), false),
    ];
    for (name, header, folder, want) in cases {
        assert_eq!(chips_go_over(&header, folder), want, "{name}");
    }
}
