use super::*;
use crate::error::CoreError;
use crate::vocab::ids::TabId;
use crate::vocab::text::RelPath;
use crate::worktree::git::GitOutput;
use crate::worktree::model::Oid;
use crate::worktree::snapshot::Snapshot;

fn oid(n: u8) -> Oid {
    Oid::parse(&format!("{n:040x}")).expect("oid")
}

fn rel(p: &str) -> RelPath {
    RelPath(p.to_owned())
}

fn out(code: i32, stdout: &[u8]) -> GitOutput {
    GitOutput {
        code,
        stdout: stdout.to_vec(),
        stderr: "fatal: bad".into(),
    }
}

#[test]
fn merge_tree_output_is_read_by_exit_code_and_a_leading_tree_id() {
    let tree = format!("{:040x}", 7);
    let clean = format!("{tree}\0");
    let conflict = format!("{tree}\0b.txt\0a.txt\0b.txt\0\0CONFLICT (content): ...\0");
    let bad_ref = b"fatal: not something we can merge\n";
    #[rustfmt::skip]
    let cases: Vec<(&str, GitOutput, Result<Vec<RelPath>, ()>)> = vec![
        ("clean", out(0, clean.as_bytes()), Ok(vec![])),
        ("conflict: sorted, deduplicated, messages ignored", out(1, conflict.as_bytes()), Ok(vec![rel("a.txt"), rel("b.txt")])),
        ("exit 1 for a bad ref is not a conflict", out(1, bad_ref), Err(())),
        ("exit 1 with a tree id and no paths is odd", out(1, clean.as_bytes()), Err(())),
        ("any other exit is a failure", out(128, b""), Err(())),
    ];
    for (name, output, want) in cases {
        assert_eq!(parse_merge_tree(&output).map_err(|_| ()), want, "{name}");
    }
}

#[test]
fn name_lists_and_counts_parse() {
    assert_eq!(parse_names(b"b\0a\0b\0"), vec![rel("a"), rel("b")]);
    assert_eq!(
        intersect(&[rel("a"), rel("c")], &[rel("b"), rel("c")]),
        vec![rel("c")]
    );
    assert_eq!(parse_counts("3\t4\n"), Ok((3, 4)));
    assert!(matches!(
        parse_counts("3\n"),
        Err(CoreError::GitOutput { .. })
    ));
}

fn snap(n: u8) -> Snapshot {
    Snapshot {
        commit: oid(n + 100),
        tree: oid(n),
    }
}

fn run(state: OverlapState, input: OverlapIn) -> (OverlapState, Vec<OverlapOut>) {
    overlap_step(state, input)
}

fn jobs(outs: &[OverlapOut]) -> Vec<&ProbeJob> {
    outs.iter()
        .filter_map(|o| match o {
            OverlapOut::Run(job) => Some(job),
            OverlapOut::Updated(_) => None,
        })
        .collect()
}

#[test]
fn only_pairs_whose_trees_changed_are_probed_again() {
    let (a, b) = (TabId(1), TabId(2));
    let (s, _) = run(OverlapState::default(), OverlapIn::BaseTip(oid(50)));
    let (s, o) = run(s, OverlapIn::Snapshotted(a, snap(1)));
    assert_eq!(jobs(&o).len(), 1, "alone: only the base");
    let (s, o) = run(s, OverlapIn::Snapshotted(b, snap(2)));
    assert_eq!(jobs(&o).len(), 2, "the pair and b's base");

    let (s, o) = run(s, OverlapIn::Snapshotted(b, snap(2)));
    assert!(o.is_empty(), "the same tree asks for nothing");

    let (s, o) = run(s, OverlapIn::Snapshotted(b, snap(3)));
    assert_eq!(jobs(&o).len(), 2, "b changed: the new pair and base only");

    let (_, o) = run(s, OverlapIn::BaseTip(oid(51)));
    assert_eq!(
        jobs(&o).len(),
        2,
        "a moved base asks about every agent again"
    );
}

#[test]
fn findings_are_red_before_amber_and_only_what_is_known() {
    let (a, b) = (TabId(1), TabId(2));
    let (s, _) = run(OverlapState::default(), OverlapIn::BaseTip(oid(50)));
    let (s, _) = run(s, OverlapIn::Snapshotted(a, snap(1)));
    let (s, o) = run(s, OverlapIn::Snapshotted(b, snap(2)));
    let ProbeJob::Pair { key, .. } = jobs(&o)[0].clone() else {
        panic!("a pair job first, got {o:?}");
    };
    assert_eq!(
        overlap_for(&s, a),
        AgentOverlap::default(),
        "nothing known yet"
    );

    let pair = PairProbe {
        conflicts: vec![rel("a.rs")],
        same_file: vec![rel("a.rs"), rel("b.rs")],
    };
    let (s, o) = run(s, OverlapIn::Pair(key, pair));
    assert!(o.contains(&OverlapOut::Updated(a)) && o.contains(&OverlapOut::Updated(b)));

    let key = BaseKey::for_test(&snap(1), &oid(50));
    let base = BaseProbe {
        conflicts: vec![],
        base_files: vec![rel("b.rs"), rel("z.rs")],
        agent_files: vec![rel("b.rs")],
        ahead: 2,
        behind: 5,
    };
    let (s, _) = run(s, OverlapIn::Base(key, base));
    let shown = overlap_for(&s, a);
    assert_eq!(
        shown.findings,
        vec![
            Finding::Conflicts {
                with: Other::Tab(b),
                file: rel("a.rs")
            },
            Finding::BothTouch {
                with: Other::Tab(b),
                file: rel("b.rs")
            },
            Finding::BaseMoved {
                files: 2,
                overlap: 1
            },
        ]
    );
    assert_eq!(
        shown.divergence,
        Some(Divergence {
            ahead: 2,
            behind: 5,
            files_changed: 1
        })
    );
    let names = |o: Other| match o {
        Other::Base => "main".to_owned(),
        Other::Tab(t) => format!("tab {}", t.0),
    };
    let lines: Vec<String> = shown.findings.iter().map(|f| f.text(names)).collect();
    assert_eq!(
        lines,
        [
            "Conflicts with tab 2 in a.rs",
            "Both touch b.rs",
            "Base moved: 2 files, 1 overlap yours"
        ]
    );
    assert_eq!(shown.findings[0].severity(), Severity::Red);
    assert_eq!(shown.findings[1].severity(), Severity::Amber);

    let (s, _) = run(s, OverlapIn::Gone(b));
    assert!(
        overlap_for(&s, a)
            .findings
            .iter()
            .all(|f| matches!(f, Finding::BaseMoved { .. }))
    );
}
