//! The shortstat and branch parsers, the Node readers, the venv name and the recompute rule.

use super::*;

fn stat(files: u32, added: u32, removed: u32) -> Option<DiffStat> {
    Some(DiffStat {
        files,
        added,
        removed,
    })
}

#[test]
fn shortstat_lines() {
    #[rustfmt::skip]
    let cases: Vec<(&str, &str, Option<DiffStat>)> = vec![
        ("both counts", " 3 files changed, 10 insertions(+), 2 deletions(-)\n", stat(3, 10, 2)),
        ("one file, singular words", " 1 file changed, 1 insertion(+), 1 deletion(-)\n", stat(1, 1, 1)),
        ("only insertions", " 2 files changed, 7 insertions(+)\n", stat(2, 7, 0)),
        ("only deletions", " 1 file changed, 4 deletions(-)\n", stat(1, 0, 4)),
        ("no changes", "", stat(0, 0, 0)),
        ("blank lines only", "\n\n", stat(0, 0, 0)),
        ("not a stat line", "fatal: not a git repository\n", None),
    ];
    for (name, output, want) in cases {
        assert_eq!(parse_shortstat(output), want, "{name}");
    }
}

#[test]
fn branch_lines() {
    #[rustfmt::skip]
    let cases: Vec<(&str, &str, Option<&str>)> = vec![
        ("plain", "main\n", Some("main")),
        ("slashes", "feature/chips\n", Some("feature/chips")),
        ("detached", "HEAD\n", None),
        ("empty", "", None),
    ];
    for (name, output, want) in cases {
        assert_eq!(
            parse_branch(output).as_ref().map(BranchName::as_str),
            want,
            "{name}"
        );
    }
}

#[test]
fn venv_names() {
    #[rustfmt::skip]
    let cases: Vec<(&str, &str, Option<&str>)> = vec![
        ("named env", "/home/p/envs/tools", Some("tools")),
        ("trailing slash", "/home/p/envs/tools/", Some("tools")),
        (".venv names the project", "/home/p/proj/.venv", Some("proj")),
        ("venv names the project", "/home/p/proj/venv", Some("proj")),
        ("bare .venv", ".venv", Some(".venv")),
        ("empty", "", None),
    ];
    for (name, path, want) in cases {
        assert_eq!(
            venv_name(path).as_ref().map(EnvName::as_str),
            want,
            "{name}"
        );
    }
}

#[test]
fn node_versions() {
    #[rustfmt::skip]
    let nvmrc: Vec<(&str, &str, Option<&str>)> = vec![
        ("plain", "v18.19.0\n", Some("v18.19.0")),
        ("comment first", "# pinned\n20\n", Some("20")),
        ("alias", "lts/*\n", Some("lts/*")),
        ("blank", "\n  \n", None),
        ("too long for a chip", "this-is-a-very-long-alias-name\n", None),
    ];
    for (name, text, want) in nvmrc {
        assert_eq!(
            node_from_nvmrc(text).as_ref().map(NodeVersion::as_str),
            want,
            "nvmrc {name}"
        );
    }
    #[rustfmt::skip]
    let engines: Vec<(&str, &str, Option<&str>)> = vec![
        ("range", r#"{"engines":{"node":">=18"}}"#, Some(">=18")),
        ("no engines", r#"{"name":"x"}"#, None),
        ("engines without node", r#"{"engines":{"npm":"9"}}"#, None),
        ("node not text", r#"{"engines":{"node":18}}"#, None),
        ("not json", "{ nope", None),
    ];
    for (name, text, want) in engines {
        assert_eq!(
            node_from_engines(text).as_ref().map(NodeVersion::as_str),
            want,
            "engines {name}"
        );
    }
}

fn state(flight: Flight, prompted: bool) -> FactsState {
    FactsState { flight, prompted }
}

#[test]
fn recompute_rule() {
    use FactsIn::{CommandDone, FolderChanged, Prompt, RunDone};
    use FactsOut::{Run, Wait};
    use Flight::{Idle, Running, Stale};
    #[rustfmt::skip]
    let cases: Vec<(&str, FactsState, FactsIn, FactsState, FactsOut)> = vec![
        ("folder change starts a run", state(Idle, false), FolderChanged, state(Running, false), Run),
        ("command done starts a run", state(Idle, true), CommandDone, state(Running, true), Run),
        ("first prompt starts a run", state(Idle, false), Prompt, state(Running, true), Run),
        ("later prompts do nothing", state(Idle, true), Prompt, state(Idle, true), Wait),
        ("trigger during a run is remembered", state(Running, true), CommandDone, state(Stale, true), Wait),
        ("many triggers coalesce", state(Stale, true), FolderChanged, state(Stale, true), Wait),
        ("first prompt during a run is remembered", state(Running, false), Prompt, state(Stale, true), Wait),
        ("done with nothing new goes idle", state(Running, true), RunDone, state(Idle, true), Wait),
        ("done with something new runs once more", state(Stale, true), RunDone, state(Running, true), Run),
        ("stray done is harmless", state(Idle, true), RunDone, state(Idle, true), Wait),
    ];
    for (name, from, input, to, out) in cases {
        assert_eq!(facts_step(from, input), (to, out), "{name}");
    }
}

#[test]
fn summary_shows_the_branch_and_a_dot_for_changes() {
    let branch = Some(BranchName("main".to_owned()));
    #[rustfmt::skip]
    let cases: Vec<(&str, Facts, Option<&str>)> = vec![
        ("clean", Facts { branch: branch.clone(), changes: stat(0, 0, 0), ..Facts::default() }, Some("main")),
        ("dirty", Facts { branch: branch.clone(), changes: stat(1, 2, 0), ..Facts::default() }, Some("main \u{2022}")),
        ("changes unknown", Facts { branch, ..Facts::default() }, Some("main")),
        ("not a repository", Facts::default(), None),
    ];
    for (name, facts, want) in cases {
        assert_eq!(facts.summary().as_deref(), want, "{name}");
    }
}
