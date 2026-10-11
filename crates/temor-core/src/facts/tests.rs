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

#[test]
fn tool_version_files() {
    type Want<'a> = (
        Option<&'a str>,
        Option<&'a str>,
        Option<&'a str>,
        Option<&'a str>,
    );
    #[rustfmt::skip]
    let cases: Vec<(&str, &str, Want)> = vec![
        ("all four", "nodejs 20.11.0\npython 3.12.1\nruby 3.3.0\ngolang 1.22.0\n",
            (Some("20.11.0"), Some("3.12.1"), Some("3.3.0"), Some("1.22.0"))),
        ("short names", "node 18\ngo 1.21\n", (Some("18"), None, None, Some("1.21"))),
        ("first of several versions", "python 3.12.1 3.11.7\n", (None, Some("3.12.1"), None, None)),
        ("comments and blanks", "# tools\n\nruby 3.2.2 # pinned\n", (None, None, Some("3.2.2"), None)),
        ("system and refs say nothing", "node system\npython ref:main\nruby path:/x\n", (None, None, None, None)),
        ("first line of a name wins", "node 20\nnode 18\n", (Some("20"), None, None, None)),
        ("other tools are ignored", "terraform 1.7.0\n", (None, None, None, None)),
        ("name alone", "node\n", (None, None, None, None)),
    ];
    for (name, text, (node, python, ruby, go)) in cases {
        let got = tool_versions(text);
        assert_eq!(
            got.node.as_ref().map(NodeVersion::as_str),
            node,
            "{name}: node"
        );
        assert_eq!(
            got.python.as_ref().map(ToolVersion::as_str),
            python,
            "{name}: python"
        );
        assert_eq!(
            got.ruby.as_ref().map(ToolVersion::as_str),
            ruby,
            "{name}: ruby"
        );
        assert_eq!(got.go.as_ref().map(ToolVersion::as_str), go, "{name}: go");
    }
    assert_eq!(
        node_from_version_file("v22.1.0\n")
            .as_ref()
            .map(NodeVersion::as_str),
        Some("v22.1.0"),
        ".node-version reads like .nvmrc"
    );
}

#[test]
fn untracked_counts() {
    let many: String = (0..1500).map(|n| format!("?? f{n}\n")).collect();
    #[rustfmt::skip]
    let cases: Vec<(&str, String, u32)> = vec![
        ("none", String::new(), 0),
        ("only tracked changes", " M a.txt\nA  b.txt\n".to_owned(), 0),
        ("mixed", " M a.txt\n?? new.txt\n?? dir/x\n".to_owned(), 2),
        ("a name that starts with ??", "?? ?? odd\n".to_owned(), 1),
        ("capped past the most", many, MOST_UNTRACKED + 1),
    ];
    for (name, output, want) in cases {
        assert_eq!(parse_untracked(&output), Untracked(want), "{name}");
    }
}

#[test]
fn chips_stop_where_the_typing_reaches() {
    // Ten-pixel cells on an 800-pixel pane with 10 pixels of padding. Chips of 4, 6 and 8
    // characters are 60, 80 and 100 pixels wide with 10-pixel gaps. All three start at 530,
    // the first two at 640, the first alone at 730; the cursor needs one cell (10) of room.
    let row = ChipRow {
        pane_width: 800.0,
        inset: 10.0,
        cell_width: 10.0,
    };
    let chars = [4, 6, 8];
    #[rustfmt::skip]
    let cases: Vec<(&str, Option<f32>, usize)> = vec![
        ("cursor on another row", None, 3),
        ("short typing", Some(100.0), 3),
        ("just clear of all three", Some(520.0), 3),
        ("one pixel in the way drops the last chip", Some(521.0), 2),
        ("just clear of two", Some(630.0), 2),
        ("two in the way leaves one", Some(631.0), 1),
        ("just clear of the first", Some(720.0), 1),
        ("under the first chip too", Some(721.0), 0),
        ("past the right edge", Some(900.0), 0),
    ];
    for (name, cursor, want) in cases {
        assert_eq!(chips_that_fit(&chars, row, cursor), want, "{name}");
    }
    assert_eq!(chips_that_fit(&[], row, Some(5.0)), 0, "no chips");
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
fn python_versions() {
    #[rustfmt::skip]
    let version_files: Vec<(&str, &str, Option<&str>)> = vec![
        ("plain", "3.12.1\n", Some("3.12.1")),
        ("comment first", "# pinned\n3.11\n3.10\n", Some("3.11")),
        ("system", "system\n", None),
        ("blank", "\n", None),
    ];
    for (name, text, want) in version_files {
        assert_eq!(
            python_from_version_file(text)
                .as_ref()
                .map(ToolVersion::as_str),
            want,
            ".python-version {name}"
        );
    }
    #[rustfmt::skip]
    let pyprojects: Vec<(&str, &str, Option<&str>)> = vec![
        ("range", "[project]\nname = \"x\"\nrequires-python = \">=3.10\"\n", Some(">=3.10")),
        ("spaces and a comment", "[project]\nrequires-python = '>= 3.9, < 4'  # note\n", Some(">=3.9,<4")),
        ("another table", "[tool.x]\nrequires-python = \">=3.8\"\n", None),
        ("table ends first", "[project]\nname = \"x\"\n[tool.y]\nrequires-python = \">=3.8\"\n", None),
        ("not quoted", "[project]\nrequires-python = 3.10\n", None),
        ("too long for a chip", "[project]\nrequires-python = \">=3.10,!=3.11.1,!=3.11.2\"\n", None),
        ("no file contents", "", None),
        ("poetry", "[tool.poetry.dependencies]\npython = \"^3.10\"\nrequests = \"^2\"\n", Some("^3.10")),
        ("poetry with spaces", "[tool.poetry.dependencies]\npython = \">= 3.9, < 4\"\n", Some(">=3.9,<4")),
        ("project wins over poetry", "[tool.poetry.dependencies]\npython = \"^3.8\"\n[project]\nrequires-python = \">=3.11\"\n", Some(">=3.11")),
        ("poetry dev group is not it", "[tool.poetry.group.dev.dependencies]\npython = \"^3.8\"\n", None),
        ("poetry table form", "[tool.poetry.dependencies]\npython = { version = \"^3.10\" }\n", None),
    ];
    for (name, text, want) in pyprojects {
        assert_eq!(
            python_from_pyproject(text)
                .as_ref()
                .map(ToolVersion::as_str),
            want,
            "pyproject {name}"
        );
    }
}
