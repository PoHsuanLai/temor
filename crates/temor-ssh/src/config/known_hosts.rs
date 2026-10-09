//! Host names from `known_hosts`, for suggestions in ⌘T. Hashed entries (`|1|…`) cannot be
//! read back, so they are skipped, as are wildcard, negated and certificate-authority lines.

use std::collections::HashSet;

use super::model::HostName;

/// The concrete host names in `text`, deduplicated in first-seen order. `[host]:port` gives
/// `host`.
pub fn known_host_names(text: &str) -> Vec<HostName> {
    let mut names: Vec<HostName> = Vec::new();
    let mut seen: HashSet<HostName> = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
            continue;
        }
        let Some(field) = line.split_whitespace().next() else {
            continue;
        };
        if field.starts_with('|') {
            continue;
        }
        for entry in field.split(',') {
            if let Some(name) = plain_host(entry).filter(|name| seen.insert(name.clone())) {
                names.push(name);
            }
        }
    }
    names
}

fn plain_host(entry: &str) -> Option<HostName> {
    if entry.starts_with('!') || entry.contains(['*', '?']) {
        return None;
    }
    let host = match entry.strip_prefix('[') {
        Some(bracketed) => bracketed.split_once(']')?.0,
        None => entry,
    };
    HostName::new(host)
}
