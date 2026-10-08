//! The bar's fixed ranking (§7, D11): the Run row when the first word is on `PATH`, then local
//! matches, then plugin rows, then AI rows, always last.

use super::model::{
    BarAction, BarRow, Candidate, PathResolver, PluginRow, RowGroup, RowKey, Target,
};
use crate::vocab::ai::AiSwitch;
use crate::vocab::text::CommandText;
use std::collections::BTreeSet;

/// How well a title matches the query; better first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Fit {
    /// The whole title, ignoring case.
    Whole,
    /// The title's start.
    Prefix,
    /// The start of a word in the title.
    WordStart,
    /// Anywhere in the title.
    Inside,
}

/// The rows for `query`, ranked.
pub fn rank(
    query: &str,
    candidates: &[Candidate],
    plugins: &[PluginRow],
    resolver: &dyn PathResolver,
    ai: AiSwitch,
) -> Vec<BarRow> {
    let query = query.trim();
    let mut rows = Vec::new();
    rows.extend(run_row(query, resolver));
    rows.extend(matches(query, candidates));
    rows.extend(plugins.iter().map(plugin_row));
    if ai == AiSwitch::On && !query.is_empty() {
        rows.extend(ai_rows(query));
    }
    let mut seen = BTreeSet::new();
    rows.retain(|row| seen.insert(row.key.clone()));
    rows
}

fn run_row(query: &str, resolver: &dyn PathResolver) -> Option<BarRow> {
    let program = query.split_whitespace().next()?;
    resolver.resolves(program).then(|| BarRow {
        key: RowKey("run".to_string()),
        group: RowGroup::Run,
        title: format!("Run `{query}` in a new tab"),
        action: BarAction::Run(CommandText(query.to_string())),
    })
}

fn matches(query: &str, candidates: &[Candidate]) -> Vec<BarRow> {
    let needle = query.to_lowercase();
    let mut fitting: Vec<(Fit, usize, std::cmp::Reverse<u32>, &Candidate)> = candidates
        .iter()
        .filter_map(|candidate| {
            let fit = fit(&needle, &candidate.title.to_lowercase())?;
            Some((
                fit,
                kind_order(&candidate.target),
                frecency(&candidate.target),
                candidate,
            ))
        })
        .collect();
    fitting.sort_by_key(|(fit, kind, frecency, _)| (*fit, *kind, *frecency));
    fitting
        .into_iter()
        .map(|(_, _, _, candidate)| BarRow {
            key: target_key(&candidate.target),
            group: RowGroup::Match,
            title: candidate.title.clone(),
            action: BarAction::Go(candidate.target.clone()),
        })
        .collect()
}

fn fit(needle: &str, title: &str) -> Option<Fit> {
    if title == needle {
        return Some(Fit::Whole);
    }
    if title.starts_with(needle) {
        return Some(Fit::Prefix);
    }
    let word_start = title
        .match_indices(needle)
        .any(|(at, _)| title[..at].ends_with(|c: char| !c.is_alphanumeric()));
    if word_start {
        return Some(Fit::WordStart);
    }
    title.contains(needle).then_some(Fit::Inside)
}

/// Where a kind of candidate sits among equally good matches: the order §7 lists them in.
fn kind_order(target: &Target) -> usize {
    match target {
        Target::Space(_) => 0,
        Target::Tab(_) => 1,
        Target::Folder(_) => 2,
        Target::Host(_) => 3,
        Target::Layout(_) => 4,
        Target::RecentCommand(_) => 5,
        Target::Directory { .. } => 6,
    }
}

fn frecency(target: &Target) -> std::cmp::Reverse<u32> {
    match target {
        Target::Directory { frecency, .. } => std::cmp::Reverse(frecency.0),
        _ => std::cmp::Reverse(0),
    }
}

fn target_key(target: &Target) -> RowKey {
    RowKey(match target {
        Target::Space(id) => format!("space:{}", id.0),
        Target::Tab(id) => format!("tab:{}", id.0),
        Target::Folder(id) => format!("folder:{}", id.0),
        Target::Host(alias) => format!("host:{}", alias.as_str()),
        Target::Layout(name) => format!("layout:{}", name.0),
        Target::RecentCommand(command) => format!("recent:{}", command.as_str()),
        Target::Directory { cwd, .. } => format!("dir:{}", cwd.as_str()),
    })
}

fn plugin_row(row: &PluginRow) -> BarRow {
    BarRow {
        key: RowKey(format!("plugin:{}:{}", row.plugin.as_str(), row.item)),
        group: RowGroup::Plugin,
        title: row.title.clone(),
        action: BarAction::Plugin {
            plugin: row.plugin.clone(),
            item: row.item.clone(),
        },
    }
}

fn ai_rows(query: &str) -> [BarRow; 3] {
    let row = |key: &str, title: String, action: BarAction| BarRow {
        key: RowKey(key.to_string()),
        group: RowGroup::Ai,
        title,
        action,
    };
    [
        row(
            "ai:suggest",
            format!("Suggest a command for: {query}"),
            BarAction::Suggest(query.to_string()),
        ),
        row(
            "ai:agent",
            format!("Ask an agent in a new pane: {query}"),
            BarAction::AskAgent(query.to_string()),
        ),
        row(
            "ai:about",
            format!("Ask about this tab: {query}"),
            BarAction::AskAbout(query.to_string()),
        ),
    ]
}
