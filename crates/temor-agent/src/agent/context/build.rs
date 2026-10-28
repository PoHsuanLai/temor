//! Building a card from what the app has: fixed defaults per entry point, nothing implicit. A
//! piece that is not there is not guessed.

use super::model::{Chip, ChipId, ChipKind, ContextCard, EntryPoint, Removal};
use temor_core::vocab::text::{CommandText, Cwd, RelPath, display_text};

/// How many of the last output lines the output chip carries.
pub const OUTPUT_LINES: usize = 40;

/// What the app can offer for a request; unset pieces are left off the card.
#[derive(Debug, Clone, Default)]
pub struct ContextSources {
    /// What the person asked.
    pub request: String,
    /// The pane's folder.
    pub cwd: Option<Cwd>,
    /// The shell's name.
    pub shell: Option<String>,
    /// The last block's command.
    pub command: Option<CommandText>,
    /// The last block's output, all of it; the card keeps the last lines.
    pub output: Option<String>,
    /// The selected text.
    pub selection: Option<String>,
    /// File names (the tab's open files, say).
    pub files: Vec<RelPath>,
}

/// The kinds each entry point offers, in order. Fixed: not chosen by any model.
fn defaults(entry: EntryPoint) -> &'static [ChipKind] {
    match entry {
        EntryPoint::FailedBlock => &[
            ChipKind::Request,
            ChipKind::Cwd,
            ChipKind::Command,
            ChipKind::Output,
        ],
        EntryPoint::SuggestCommand => &[ChipKind::Request, ChipKind::Cwd, ChipKind::Shell],
        EntryPoint::AskAboutTab => &[
            ChipKind::Request,
            ChipKind::Cwd,
            ChipKind::Selection,
            ChipKind::FileName,
        ],
    }
}

/// The card for `entry` from `sources`.
pub fn build_card(entry: EntryPoint, sources: &ContextSources) -> ContextCard {
    let mut chips = Vec::new();
    for kind in defaults(entry) {
        let pieces: Vec<(String, String)> = match kind {
            ChipKind::Request => vec![(preview(&sources.request), sources.request.clone())],
            ChipKind::Cwd => sources
                .cwd
                .iter()
                .map(|c| (preview(c.as_str()), c.0.clone()))
                .collect(),
            ChipKind::Shell => sources
                .shell
                .iter()
                .map(|s| (preview(s), s.clone()))
                .collect(),
            ChipKind::Command => sources
                .command
                .iter()
                .filter(|c| !c.0.is_empty())
                .map(|c| (preview(c.as_str()), c.0.clone()))
                .collect(),
            ChipKind::Output => sources
                .output
                .iter()
                .map(|o| last_lines(o))
                .filter(|o| !o.is_empty())
                .map(|o| (preview(&o), o))
                .collect(),
            ChipKind::Selection => sources
                .selection
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| (preview(s), s.clone()))
                .collect(),
            ChipKind::FileName => sources
                .files
                .iter()
                .map(|f| (preview(f.as_str()), f.0.clone()))
                .collect(),
        };
        for (label, text) in pieces {
            let removal = match kind {
                ChipKind::Request => Removal::Fixed,
                _ => Removal::Removable,
            };
            let id = ChipId(u32::try_from(chips.len()).unwrap_or(u32::MAX));
            chips.push(Chip {
                id,
                kind: *kind,
                label,
                text,
                removal,
            });
        }
    }
    ContextCard {
        entry,
        chips,
        removed: Vec::new(),
    }
}

fn last_lines(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let from = lines.len().saturating_sub(OUTPUT_LINES);
    lines[from..].join("\n")
}

/// The first line, cleaned for display and cut to 40 characters.
fn preview(text: &str) -> String {
    let first = text.lines().next().unwrap_or("");
    let shown = display_text(first, 40);
    match first.chars().count() > 40 || text.lines().nth(1).is_some() {
        true => format!("{shown}…"),
        false => shown,
    }
}
