//! The facts edge on scratch folders and a canned git: what is read, in which order, and what is
//! left out when something fails.

use std::path::Path;
use std::sync::Mutex;

use temor_core::error::CoreError;
use temor_core::facts::{DiffStat, Untracked};
use temor_core::worktree::{Git, GitCommand, GitOutput};

use super::{FactsEnv, read_facts};

/// Answers each git call from a list of (first argument, exit code, output); remembers the calls.
struct Canned {
    answers: Vec<(&'static str, i32, &'static str)>,
    seen: Mutex<Vec<GitCommand>>,
}

impl Canned {
    fn new(answers: Vec<(&'static str, i32, &'static str)>) -> Canned {
        Canned {
            answers,
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl Git for Canned {
    fn run(&self, cmd: &GitCommand) -> Result<GitOutput, CoreError> {
        self.seen.lock().expect("lock").push(cmd.clone());
        let first = cmd.args.first().map_or("", String::as_str);
        let (_, code, out) = self
            .answers
            .iter()
            .find(|(verb, _, _)| *verb == first)
            .copied()
            .unwrap_or(("", 128, ""));
        Ok(GitOutput {
            code,
            stdout: out.as_bytes().to_vec(),
            stderr: String::new(),
        })
    }
}

fn dir(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// (name, git answers as (args, status, stdout), expected branch, changes, untracked)
type Case = (
    &'static str,
    Vec<(&'static str, i32, &'static str)>,
    Option<&'static str>,
    Option<DiffStat>,
    Option<Untracked>,
);

#[test]
fn git_facts_come_from_a_few_runs_without_locks_and_fail_quietly() {
    #[rustfmt::skip]
    let cases: Vec<Case> = vec![
        ("repository with changes",
            vec![("rev-parse", 0, "main\n"), ("diff", 0, " 2 files changed, 5 insertions(+), 1 deletion(-)\n"),
                 ("status", 0, " M a\n M b\n?? c\n?? d\n?? e\n")],
            Some("main"), Some(DiffStat { files: 2, added: 5, removed: 1 }), Some(Untracked(3))),
        ("clean repository",
            vec![("rev-parse", 0, "dev\n"), ("diff", 0, ""), ("status", 0, "")],
            Some("dev"), Some(DiffStat::default()), Some(Untracked(0))),
        ("not a repository",
            vec![("rev-parse", 128, ""), ("symbolic-ref", 128, ""), ("diff", 0, " 1 file changed, 1 insertion(+)\n"), ("status", 0, "?? x\n")],
            None, None, None),
        ("diff fails, branch stays",
            vec![("rev-parse", 0, "main\n"), ("diff", 128, ""), ("status", 0, "")],
            Some("main"), None, Some(Untracked(0))),
        ("no commit yet shows its branch",
            vec![("rev-parse", 128, ""), ("symbolic-ref", 0, "trunk\n"), ("diff", 128, ""), ("status", 0, "?? a\n?? b\n")],
            Some("trunk"), None, Some(Untracked(2))),
        ("detached head has no branch",
            vec![("rev-parse", 0, "HEAD\n"), ("symbolic-ref", 128, ""), ("status", 0, "?? a\n")],
            None, None, None),
        ("status fails, the rest stays",
            vec![("rev-parse", 0, "main\n"), ("diff", 0, ""), ("status", 128, "")],
            Some("main"), Some(DiffStat::default()), None),
    ];
    let scratch = tempfile::tempdir().expect("tempdir");
    for (name, answers, branch, changes, untracked) in cases {
        let git = Canned::new(answers);
        let facts = read_facts(Some(&git), &dir(scratch.path()), &FactsEnv::default());
        assert_eq!(facts.branch.as_ref().map(|b| b.as_str()), branch, "{name}");
        assert_eq!(facts.changes, changes, "{name}");
        assert_eq!(facts.untracked, untracked, "{name}");
        let seen = git.seen.lock().expect("lock");
        assert!(
            seen.iter()
                .all(|c| c.env == [("GIT_OPTIONAL_LOCKS".to_owned(), "0".to_owned())]),
            "{name}: every run takes no locks"
        );
    }
}

#[test]
fn venv_comes_from_the_environment_first_then_the_folder() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let here = dir(scratch.path());
    let from_env = FactsEnv {
        virtual_env: Some("/envs/tools".to_owned()),
    };
    let name = |env: &FactsEnv| {
        read_facts(None, &here, env)
            .venv
            .map(|v| v.as_str().to_owned())
    };
    assert_eq!(name(&FactsEnv::default()), None, "nothing set, no folder");
    std::fs::create_dir(scratch.path().join(".venv")).expect("mkdir");
    let project = scratch
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    assert_eq!(
        name(&FactsEnv::default()),
        project,
        "a .venv folder names the project"
    );
    assert_eq!(
        name(&from_env),
        Some("tools".to_owned()),
        "the environment wins"
    );
}

#[test]
fn node_comes_from_the_nearest_file_up_to_the_repository_root() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let root = scratch.path();
    let deep = root.join("app/src");
    std::fs::create_dir_all(&deep).expect("mkdir");
    let node = |at: &Path| {
        read_facts(None, &dir(at), &FactsEnv::default())
            .node
            .map(|n| n.as_str().to_owned())
    };
    assert_eq!(node(&deep), None, "nothing anywhere");
    std::fs::write(root.join("package.json"), r#"{"engines":{"node":">=18"}}"#).expect("write");
    assert_eq!(node(&deep), Some(">=18".to_owned()), "found in a parent");
    std::fs::write(root.join("app/.nvmrc"), "v20\n").expect("write");
    assert_eq!(node(&deep), Some("v20".to_owned()), "the nearer file wins");
    // A repository root ends the search: a file above it belongs to another project.
    let inner = tempfile::tempdir_in(root).expect("tempdir");
    std::fs::create_dir(inner.path().join(".git")).expect("mkdir");
    assert_eq!(node(inner.path()), None, "stops at the repository root");
}

#[test]
fn a_version_file_over_the_cap_is_not_read_here_as_on_a_host() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let root = scratch.path();
    let node = || {
        read_facts(None, &dir(root), &FactsEnv::default())
            .node
            .map(|n| n.as_str().to_owned())
    };
    std::fs::write(root.join(".nvmrc"), "v20\n").expect("write");
    assert_eq!(node(), Some("v20".to_owned()));
    let padding = " ".repeat(usize::try_from(temor_wire::message::MOST_FACT_BYTES).unwrap_or(0));
    std::fs::write(root.join(".nvmrc"), format!("v20\n{padding}")).expect("write");
    assert_eq!(node(), None, "over the cap reads as no file, as on a host");
}

#[test]
fn runtime_versions_come_from_the_nearest_files() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let root = scratch.path();
    let deep = root.join("app");
    std::fs::create_dir_all(&deep).expect("mkdir");
    let versions = |at: &Path| {
        let f = read_facts(None, &dir(at), &FactsEnv::default());
        let text = |v: Option<&str>| v.map(str::to_owned);
        (
            text(f.node.as_ref().map(|v| v.as_str())),
            text(f.python.as_ref().map(|v| v.as_str())),
            text(f.ruby.as_ref().map(|v| v.as_str())),
            text(f.go.as_ref().map(|v| v.as_str())),
        )
    };
    let s = |v: &str| Some(v.to_owned());
    assert_eq!(
        versions(&deep),
        (None, None, None, None),
        "nothing anywhere"
    );
    std::fs::write(
        root.join(".tool-versions"),
        "nodejs 18.0.0\npython 3.12.1\nruby 3.3.0\ngolang 1.22.0\n",
    )
    .expect("write");
    assert_eq!(
        versions(&deep),
        (s("18.0.0"), s("3.12.1"), s("3.3.0"), s("1.22.0")),
        "all from one file in a parent"
    );
    std::fs::write(deep.join(".node-version"), "v20\n").expect("write");
    assert_eq!(
        versions(&deep).0,
        s("v20"),
        ".node-version in the nearer folder wins"
    );
    std::fs::write(deep.join(".nvmrc"), "v21\n").expect("write");
    assert_eq!(
        versions(&deep).0,
        s("v21"),
        ".nvmrc comes before .node-version"
    );
    std::fs::write(deep.join(".tool-versions"), "python 3.9.0\n").expect("write");
    let got = versions(&deep);
    assert_eq!(
        got.1,
        s("3.9.0"),
        "the nearer .tool-versions wins for python"
    );
    assert_eq!(got.2, s("3.3.0"), "ruby still comes from the parent");
}

#[test]
fn python_comes_from_its_own_files_and_pyproject_only_fills_a_gap() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let root = scratch.path();
    let python = |at: &Path| {
        read_facts(None, &dir(at), &FactsEnv::default())
            .python
            .map(|v| v.as_str().to_owned())
    };
    let s = |v: &str| Some(v.to_owned());
    let write = |name: &str, text: &str| std::fs::write(root.join(name), text).expect("write");
    assert_eq!(python(root), None, "nothing anywhere");
    write(
        "pyproject.toml",
        "[project]\nrequires-python = \">=3.10\"\n",
    );
    assert_eq!(
        python(root),
        s(">=3.10"),
        "pyproject when nothing else says"
    );
    write(".tool-versions", "python 3.9.0\n");
    assert_eq!(python(root), s("3.9.0"), ".tool-versions beats pyproject");
    write(".python-version", "3.12.1\n");
    assert_eq!(python(root), s("3.12.1"), ".python-version comes first");
    let deep = root.join("pkg");
    std::fs::create_dir(&deep).expect("mkdir");
    std::fs::write(
        deep.join("pyproject.toml"),
        "[project]\nrequires-python = \">=3.8\"\n",
    )
    .expect("write");
    assert_eq!(
        python(&deep),
        s("3.12.1"),
        "a version file above beats a nearer pyproject"
    );
}

#[test]
fn untracked_counting_stops_past_the_most_shown() {
    let tracked: String = (0..1200).map(|n| format!(" M t{n}\n")).collect();
    let new: String = (0..1500).map(|n| format!("?? f{n}\n")).collect();
    let status: &'static str = Box::leak(format!("{tracked}{new}").into_boxed_str());
    let git = Canned::new(vec![
        ("rev-parse", 0, "main\n"),
        ("diff", 0, ""),
        ("status", 0, status),
    ]);
    let scratch = tempfile::tempdir().expect("tempdir");
    let facts = read_facts(Some(&git), &dir(scratch.path()), &FactsEnv::default());
    assert_eq!(
        facts.untracked,
        Some(Untracked(1000)),
        "tracked changes do not use up the count"
    );
    let seen = git.seen.lock().expect("lock");
    let status = seen
        .iter()
        .find(|c| c.args[0] == "status")
        .expect("status ran");
    assert!(
        status.args.iter().any(|a| a == "--untracked-files=normal"),
        "{:?}",
        status.args
    );
}
