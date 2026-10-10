//! The facts edge on scratch folders and a canned git: what is read, in which order, and what is
//! left out when something fails.

use std::path::Path;
use std::sync::Mutex;

use temor_core::error::CoreError;
use temor_core::facts::DiffStat;
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

/// (name, git answers as (args, status, stdout), expected branch, expected changes)
type Case = (
    &'static str,
    Vec<(&'static str, i32, &'static str)>,
    Option<&'static str>,
    Option<DiffStat>,
);

#[test]
fn git_facts_come_from_two_runs_without_locks_and_fail_quietly() {
    #[rustfmt::skip]
    let cases: Vec<Case> = vec![
        ("repository with changes",
            vec![("rev-parse", 0, "main\n"), ("diff", 0, " 2 files changed, 5 insertions(+), 1 deletion(-)\n")],
            Some("main"), Some(DiffStat { files: 2, added: 5, removed: 1 })),
        ("clean repository",
            vec![("rev-parse", 0, "dev\n"), ("diff", 0, "")],
            Some("dev"), Some(DiffStat::default())),
        ("not a repository",
            vec![("rev-parse", 128, ""), ("diff", 0, " 1 file changed, 1 insertion(+)\n")],
            None, None),
        ("diff fails, branch stays",
            vec![("rev-parse", 0, "main\n"), ("diff", 128, "")],
            Some("main"), None),
    ];
    let scratch = tempfile::tempdir().expect("tempdir");
    for (name, answers, branch, changes) in cases {
        let git = Canned::new(answers);
        let facts = read_facts(Some(&git), &dir(scratch.path()), &FactsEnv::default());
        assert_eq!(facts.branch.as_ref().map(|b| b.as_str()), branch, "{name}");
        assert_eq!(facts.changes, changes, "{name}");
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
