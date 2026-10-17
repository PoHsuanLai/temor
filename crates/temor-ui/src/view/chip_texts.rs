//! The words of the context chips (PLAN P16): which chips a folder's facts make, and the
//! folder and branch chips that lead the row over temor's header line. How they are placed and
//! drawn is `fact_chips`.

use temor_core::facts::{DiffStat, Facts, MOST_UNTRACKED};

use super::fact_chips::{ChipIcon, ChipKind, ChipPart, ChipText, Tone};
use crate::plugins::facts::{PluginFact, chip_words};

fn part(text: impl Into<String>, tone: Tone) -> ChipPart {
    ChipPart {
        text: text.into(),
        tone,
    }
}

fn chip(kind: ChipKind, icon: Option<ChipIcon>, parts: Vec<ChipPart>) -> ChipText {
    ChipText { kind, icon, parts }
}

/// A tool's chip: its name in the chip's colour, then what it is set to.
fn tool(kind: ChipKind, name: &str, value: &str) -> ChipText {
    chip(
        kind,
        None,
        vec![part(name, Tone::Strong), part(value, Tone::Plain)],
    )
}

fn files_words(files: u32) -> String {
    match files {
        1 => "1 file".to_owned(),
        n => format!("{n} files"),
    }
}

fn untracked_words(count: u32) -> String {
    match count > MOST_UNTRACKED {
        true => format!("{MOST_UNTRACKED}+ new"),
        false => format!("{count} new"),
    }
}

/// The chips for `facts`, in order: the changes (`+N -M · F files`), new files, Python (the
/// environment, else the version), Ruby, Go, Node. Nothing known gives none; a clean folder gives
/// no change chip.
pub fn chip_texts(facts: &Facts) -> Vec<ChipText> {
    let mut chips = Vec::new();
    if let Some(DiffStat {
        files,
        added,
        removed,
    }) = facts.changes.filter(|s| s.is_dirty())
    {
        chips.push(chip(
            ChipKind::Changes,
            Some(ChipIcon::Changes),
            vec![
                part(format!("+{added}"), Tone::Added),
                part(format!("\u{2212}{removed}"), Tone::Removed),
                part("\u{b7}", Tone::Plain),
                part(files_words(files), Tone::Plain),
            ],
        ));
    }
    if let Some(untracked) = facts.untracked.filter(|u| u.0 > 0) {
        chips.push(chip(
            ChipKind::New,
            None,
            vec![part(untracked_words(untracked.0), Tone::Plain)],
        ));
    }
    let python = match (&facts.venv, &facts.python) {
        (Some(venv), _) => Some(venv.as_str()),
        (None, Some(version)) => Some(version.as_str()),
        (None, None) => None,
    };
    if let Some(python) = python {
        chips.push(tool(ChipKind::Python, "python", python));
    }
    if let Some(ruby) = &facts.ruby {
        chips.push(tool(ChipKind::Ruby, "ruby", ruby.as_str()));
    }
    if let Some(go) = &facts.go {
        chips.push(tool(ChipKind::Go, "go", go.as_str()));
    }
    if let Some(node) = &facts.node {
        chips.push(tool(ChipKind::Node, "node", node.as_str()));
    }
    chips
}

/// A chip for each fact plugins gave, in the order they gave them: the label in the chip's
/// colour, then the value.
pub fn plugin_chips(plugin: &[PluginFact]) -> Vec<ChipText> {
    plugin
        .iter()
        .map(|fact| {
            let (label, value) = chip_words(fact);
            tool(ChipKind::Plugin, label, value)
        })
        .collect()
}

/// Every chip after the folder and branch: the built-in ones, then the plugins'.
pub fn all_chips(facts: &Facts, plugin: &[PluginFact]) -> Vec<ChipText> {
    let mut chips = chip_texts(facts);
    chips.extend(plugin_chips(plugin));
    chips
}

/// The folder and branch chips that lead the row over temor's header line.
pub fn lead_chips(folder: &str, facts: &Facts) -> Vec<ChipText> {
    let mut chips = vec![chip(
        ChipKind::Folder,
        Some(ChipIcon::Folder),
        vec![part(folder, Tone::Strong)],
    )];
    if let Some(branch) = &facts.branch {
        chips.push(chip(
            ChipKind::Branch,
            Some(ChipIcon::Branch),
            vec![part(branch.as_str(), Tone::Strong)],
        ));
    }
    chips
}
