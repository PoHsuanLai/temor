//! The Node version a project asks for, from `.nvmrc` or `package.json`.

use super::model::NodeVersion;

/// The longest version text kept: it has to fit a chip.
const LONGEST: usize = 16;

fn chip(text: &str) -> Option<NodeVersion> {
    let text = text.trim();
    match text.is_empty() || text.chars().count() > LONGEST || text.contains(char::is_whitespace) {
        true => None,
        false => Some(NodeVersion(text.to_owned())),
    }
}

/// The version from the text of an `.nvmrc`: its first non-comment line. Aliases such as
/// `lts/*` are shown as written.
pub fn node_from_nvmrc(text: &str) -> Option<NodeVersion> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .and_then(chip)
}

/// The version from the text of a `package.json`: `engines.node`. Anything else in the file, or a
/// file that is not JSON, gives `None`.
pub fn node_from_engines(text: &str) -> Option<NodeVersion> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    json.get("engines")?.get("node")?.as_str().and_then(chip)
}
