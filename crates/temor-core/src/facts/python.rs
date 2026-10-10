//! The Python version a project asks for, from `.python-version` or `pyproject.toml`.

use super::model::ToolVersion;
use super::node::chip_text;

fn chip(text: &str) -> Option<ToolVersion> {
    chip_text(text).map(|t| ToolVersion(t.to_owned()))
}

/// The version from the text of a `.python-version` file (pyenv): its first non-comment line.
/// `system` says nothing a chip can show.
pub fn python_from_version_file(text: &str) -> Option<ToolVersion> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| *l != "system")
        .and_then(chip)
}

/// The version from the text of a `pyproject.toml`: `requires-python` in its `[project]` table
/// (`>=3.10`). Read line by line, not as a whole TOML file: only a quoted value on the key's own
/// line counts.
pub fn python_from_pyproject(text: &str) -> Option<ToolVersion> {
    let mut in_project = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_project = line.split('#').next().unwrap_or("").trim() == "[project]";
            continue;
        }
        if !in_project {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "requires-python" {
            continue;
        }
        let value = value.split('#').next().unwrap_or("").trim();
        let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let inner = value[1..].split(quote).next()?;
        return chip(&inner.replace(' ', ""));
    }
    None
}
