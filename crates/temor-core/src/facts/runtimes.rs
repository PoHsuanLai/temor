//! The runtimes a folder asks for, found by walking up from it through the version files.

use std::path::Path;

use super::{
    NodeVersion, ToolVersion, node_from_engines, node_from_nvmrc, node_from_version_file,
    python_from_pyproject, python_from_version_file, tool_versions,
};

/// The versions found, each from the nearest folder that names it.
#[derive(Debug, Default)]
pub struct Runtimes {
    pub node: Option<NodeVersion>,
    pub python: Option<ToolVersion>,
    pub ruby: Option<ToolVersion>,
    pub go: Option<ToolVersion>,
}

/// Where the folders' files are read from: this computer's disk, or what a host's helper sent.
pub trait FolderSource {
    /// The text of `file` in `folder`, when it is there.
    fn read(&self, folder: &Path, file: &str) -> Option<String>;
    /// Whether `folder` is the top of a repository, where the walk up stops.
    fn is_root(&self, folder: &Path) -> bool;
}

/// The versions from the nearest files, looking in `dir` and then each parent, at most `parents`
/// folders in all, up to the repository root. In one folder Node comes from `.nvmrc`, then
/// `.node-version`, then `.tool-versions`, then `package.json`'s `engines.node`; Python from
/// `.python-version`, then `.tool-versions`; Ruby and Go from `.tool-versions`. Only when no
/// folder names a Python version does the nearest `pyproject.toml`'s `requires-python` stand in.
pub fn runtimes_in(dir: &Path, source: &dyn FolderSource, parents: usize) -> Runtimes {
    let read = |folder: &Path, file: &str| source.read(folder, file);
    let mut out = Runtimes::default();
    let mut required = None;
    for folder in dir.ancestors().take(parents) {
        let tools = read(folder, ".tool-versions")
            .map(|text| tool_versions(&text))
            .unwrap_or_default();
        if out.node.is_none() {
            out.node = read(folder, ".nvmrc")
                .and_then(|t| node_from_nvmrc(&t))
                .or_else(|| read(folder, ".node-version").and_then(|t| node_from_version_file(&t)))
                .or(tools.node)
                .or_else(|| read(folder, "package.json").and_then(|t| node_from_engines(&t)));
        }
        if out.python.is_none() {
            out.python = read(folder, ".python-version")
                .and_then(|t| python_from_version_file(&t))
                .or(tools.python);
        }
        if required.is_none() {
            required = read(folder, "pyproject.toml").and_then(|t| python_from_pyproject(&t));
        }
        out.ruby = out.ruby.or(tools.ruby);
        out.go = out.go.or(tools.go);
        let all =
            out.node.is_some() && out.python.is_some() && out.ruby.is_some() && out.go.is_some();
        if all || source.is_root(folder) {
            break;
        }
    }
    out.python = out.python.or(required);
    out
}
