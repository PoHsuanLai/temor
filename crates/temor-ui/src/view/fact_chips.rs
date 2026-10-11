//! The context chips (PLAN P16): small chips at the right end of the prompt's header row, the row
//! the newest prompt is on, stating what the pane's folder is: the lines added and removed, the files changed, the Python
//! environment and the Node version. They sit beside the grid, so a press never reaches the
//! terminal, and they are not drawn on a full-screen program's screen.
//!
//! The branch is not a chip: the prompt's own header line already says it (and the tab's row).

use dioxus::prelude::*;
use ds::components::controls::chip::{Chip, ChipVariant};
use temor_core::facts::{ChipRow, DiffStat, Facts, MOST_UNTRACKED, chips_that_fit};

use crate::session::Session;

/// One chip's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipText {
    pub text: String,
}

/// Where the chips go and what they say, placed by the window from the session.
#[derive(Debug, Clone, PartialEq)]
pub struct ChipsView {
    pub chips: Vec<ChipText>,
    /// The row's top in logical pixels.
    pub top: f32,
    /// The row's height in logical pixels.
    pub height: f32,
    /// The pane's padding in logical pixels: the chips end this far from the right edge.
    pub inset: f32,
}

fn chip(text: String) -> ChipText {
    ChipText { text }
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

/// The chips for `facts`, in order: `+N -M`, files, new files, Python (the environment, else the
/// version), Ruby, Go, Node. Nothing known gives none; a clean folder gives no change chips.
pub fn chip_texts(facts: &Facts) -> Vec<ChipText> {
    let mut chips = Vec::new();
    if let Some(DiffStat {
        files,
        added,
        removed,
    }) = facts.changes.filter(|s| s.is_dirty())
    {
        chips.push(chip(format!("+{added} \u{2212}{removed}")));
        chips.push(chip(files_words(files)));
    }
    if let Some(untracked) = facts.untracked.filter(|u| u.0 > 0) {
        chips.push(chip(untracked_words(untracked.0)));
    }
    let python = match (&facts.venv, &facts.python) {
        (Some(venv), _) => Some(venv.as_str()),
        (None, Some(version)) => Some(version.as_str()),
        (None, None) => None,
    };
    if let Some(python) = python {
        chips.push(chip(format!("python {python}")));
    }
    if let Some(ruby) = &facts.ruby {
        chips.push(chip(format!("ruby {}", ruby.as_str())));
    }
    if let Some(go) = &facts.go {
        chips.push(chip(format!("go {}", go.as_str())));
    }
    if let Some(node) = &facts.node {
        chips.push(chip(format!("node {}", node.as_str())));
    }
    chips
}

/// The chips and their place for a session: on the row of the newest block's prompt, when that
/// prompt is on screen. `None` when there is nothing to say, on a full-screen program's screen, or
/// before the first layout.
pub fn chips_for(session: &Session, facts: &Facts) -> Option<ChipsView> {
    let mut chips = chip_texts(facts);
    if chips.is_empty() {
        return None;
    }
    let gutter = session.gutter_geometry()?;
    let (inset, cell_h) = (gutter.side, gutter.cell_h);
    // The prompt being typed at, when it is in view and not on the alternate screen.
    let row = session.live_prompt_row()?;
    let top = gutter.row_top(row);
    // A command typed on this very row runs under the chips: leave out the ones it reaches.
    let (pane_width, cell_width) = session.grid_logical()?;
    let cursor_right = session
        .cursor_logical()
        .filter(|[_, y, _, h]| (y - top).abs() < h / 2.0)
        .map(|[x, _, w, _]| x + w);
    let words: Vec<usize> = chips.iter().map(|c| c.text.chars().count()).collect();
    let geometry = ChipRow {
        pane_width,
        inset,
        cell_width,
    };
    chips.truncate(chips_that_fit(&words, geometry, cursor_right));
    if chips.is_empty() {
        return None;
    }
    Some(ChipsView {
        chips,
        top,
        height: cell_h,
        inset,
    })
}

/// The chips over the prompt's header row.
#[component]
#[allow(non_snake_case)]
pub fn FactChips(view: ChipsView) -> Element {
    rsx! {
        div {
            class: "temor-fact-chips",
            style: "top:{view.top}px; height:{view.height}px; padding-right:{view.inset}px",
            for (n, one) in view.chips.iter().enumerate() {
                Chip {
                    key: "{n}",
                    variant: ChipVariant::Neutral,
                    text: one.text.clone(),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "fact_chips_tests.rs"]
mod tests;
