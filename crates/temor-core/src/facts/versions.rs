//! The versions a `.tool-versions` file (asdf, mise) asks for.

use super::model::{NodeVersion, ToolVersion};
use super::node::chip_text;

/// The runtimes a `.tool-versions` file names that the chips show.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolVersions {
    pub node: Option<NodeVersion>,
    pub python: Option<ToolVersion>,
    pub ruby: Option<ToolVersion>,
    pub go: Option<ToolVersion>,
}

/// Reads `name version [fallback ...] [# comment]` lines. Only the first version of a line counts;
/// `system` and `ref:`/`path:` entries say nothing a chip can show, and a name that appears twice
/// keeps its first line.
pub fn tool_versions(text: &str) -> ToolVersions {
    let mut out = ToolVersions::default();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let mut words = line.split_whitespace();
        let (Some(name), Some(version)) = (words.next(), words.next()) else {
            continue;
        };
        if version == "system" || version.starts_with("ref:") || version.starts_with("path:") {
            continue;
        }
        let Some(version) = chip_text(version) else {
            continue;
        };
        let tool = || ToolVersion(version.to_owned());
        match name {
            "nodejs" | "node" if out.node.is_none() => {
                out.node = Some(NodeVersion(version.to_owned()))
            }
            "python" if out.python.is_none() => out.python = Some(tool()),
            "ruby" if out.ruby.is_none() => out.ruby = Some(tool()),
            "golang" | "go" if out.go.is_none() => out.go = Some(tool()),
            _ => {}
        }
    }
    out
}
