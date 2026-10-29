//! The facts edge on scratch folders: what is read, in which order, and what is left out.

use std::path::Path;

use super::{FactsEnv, read_facts};

fn dir(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn venv_comes_from_the_environment_first_then_the_folder() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let here = dir(scratch.path());
    let from_env = FactsEnv {
        virtual_env: Some("/envs/tools".to_owned()),
    };
    let name = |env: &FactsEnv| read_facts(&here, env).venv.map(|v| v.as_str().to_owned());
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
        read_facts(&dir(at), &FactsEnv::default())
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
        read_facts(&dir(root), &FactsEnv::default())
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
        let f = read_facts(&dir(at), &FactsEnv::default());
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
        read_facts(&dir(at), &FactsEnv::default())
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
