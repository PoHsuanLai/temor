//! The context chips (PLAN P16): small chips at the right end of the prompt's header row, the row
//! the newest prompt is on, stating what the pane's folder is: the git branch (with a dot when
//! there are uncommitted changes), the lines added and removed, the files changed, the Python
//! environment and the Node version. They sit beside the grid, so a press never reaches the
//! terminal, and they are not drawn on a full-screen program's screen.

use dioxus::prelude::*;
use ds::components::controls::chip::{Chip, ChipVariant};
use temor_core::facts::{DiffStat, Facts};

use crate::session::Session;

/// How a chip stands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipTone {
    Plain,
    /// The folder has uncommitted changes.
    Changed,
}

/// One chip's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipText {
    pub text: String,
    pub tone: ChipTone,
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

fn chip(text: String, tone: ChipTone) -> ChipText {
    ChipText { text, tone }
}

fn files_words(files: u32) -> String {
    match files {
        1 => "1 file".to_owned(),
        n => format!("{n} files"),
    }
}

/// The chips for `facts`, in order: branch (a dot when there are changes), `+N -M`, files,
/// environment, Node. Nothing known gives none; a clean folder gives no change chips.
pub fn chip_texts(facts: &Facts) -> Vec<ChipText> {
    let mut chips = Vec::new();
    if let Some(summary) = facts.summary() {
        let tone = match facts.dirty() {
            true => ChipTone::Changed,
            false => ChipTone::Plain,
        };
        chips.push(chip(summary, tone));
    }
    if let Some(DiffStat {
        files,
        added,
        removed,
    }) = facts.changes.filter(|s| s.is_dirty())
    {
        chips.push(chip(format!("+{added} \u{2212}{removed}"), ChipTone::Plain));
        chips.push(chip(files_words(files), ChipTone::Plain));
    }
    if let Some(venv) = &facts.venv {
        chips.push(chip(format!("python {}", venv.as_str()), ChipTone::Plain));
    }
    if let Some(node) = &facts.node {
        chips.push(chip(format!("node {}", node.as_str()), ChipTone::Plain));
    }
    chips
}

/// The chips and their place for a session: on the row of the newest block's prompt, when that
/// prompt is on screen. `None` when there is nothing to say, on a full-screen program's screen, or
/// before the first layout.
pub fn chips_for(session: &Session, facts: &Facts) -> Option<ChipsView> {
    let chips = chip_texts(facts);
    if chips.is_empty() {
        return None;
    }
    let (inset, cell_h) = session.gutter_geometry()?;
    let newest = session.block_outcomes().last().map(|(id, _)| *id)?;
    // `block_marks` is empty on the alternate screen and holds only prompts that are in view.
    let mark = session
        .block_marks()
        .into_iter()
        .find(|mark| mark.id == newest)?;
    Some(ChipsView {
        chips,
        top: inset + mark.row as f32 * cell_h,
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
                    variant: match one.tone {
                        ChipTone::Changed => ChipVariant::Accent,
                        ChipTone::Plain => ChipVariant::Neutral,
                    },
                    text: one.text.clone(),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "fact_chips_tests.rs"]
mod tests;
