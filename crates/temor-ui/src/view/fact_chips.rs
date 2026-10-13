//! The context chips (PLAN P16), as Warp draws them: a row of small rounded chips on the
//! prompt's header line saying what the pane's folder is: the folder, the git branch, the lines
//! added and removed and the files changed, new files, the Python environment and the tool
//! versions. With temor's own prompt the row stands over the header line, left to right, in place
//! of the folder and branch the shell wrote there. With a one-line prompt, or a header that says
//! something else (the person's own prompt, another computer's folder), the shell's text stays and
//! the facts' chips end at the row's right edge instead, without the folder and branch.
//!
//! They sit beside the grid, so a press never reaches the terminal, and they are not drawn on a
//! full-screen program's screen.

use dioxus::prelude::*;
use ds::prelude::{ExternalIcon, IconSize, IconSource, IconView};
use ds::style::icon::url::IconUrl;
use temor_core::facts::{ChipRow, DiffStat, Facts, MOST_UNTRACKED, chips_that_fit};

use crate::session::{HeaderLine, PromptHeader, Session};

/// The small picture at a chip's start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipIcon {
    Folder,
    Branch,
    Changes,
}

/// How a run of a chip's words is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// The chip's own colour.
    Plain,
    /// What the chip is named for (the folder, the branch), a step brighter.
    Strong,
    /// Lines added.
    Added,
    /// Lines removed.
    Removed,
}

impl Tone {
    fn slug(self) -> &'static str {
        match self {
            Tone::Plain => "plain",
            Tone::Strong => "strong",
            Tone::Added => "added",
            Tone::Removed => "removed",
        }
    }
}

/// A run of a chip's words in one colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipPart {
    pub text: String,
    pub tone: Tone,
}

/// One chip: its picture, if any, and its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipText {
    pub icon: Option<ChipIcon>,
    pub parts: Vec<ChipPart>,
}

impl ChipText {
    /// The chip's words, a space between runs (as they are drawn: the gap between runs is a
    /// space wide).
    pub fn text(&self) -> String {
        self.parts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Where on its row the chips go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipPlace {
    /// Over the whole header line, from its left: temor's prompt, whose text the chips replace.
    Over,
    /// At the right end, beside the shell's own text.
    End,
}

/// Where the chips go and what they say, placed by the window from the session.
#[derive(Debug, Clone, PartialEq)]
pub struct ChipsView {
    pub chips: Vec<ChipText>,
    pub place: ChipPlace,
    /// The row's top in logical pixels.
    pub top: f32,
    /// The row's height in logical pixels.
    pub height: f32,
    /// The pane's padding in logical pixels: the chips keep this far from the edge.
    pub inset: f32,
}

fn part(text: impl Into<String>, tone: Tone) -> ChipPart {
    ChipPart {
        text: text.into(),
        tone,
    }
}

fn chip(icon: Option<ChipIcon>, parts: Vec<ChipPart>) -> ChipText {
    ChipText { icon, parts }
}

fn plain(text: String) -> ChipText {
    chip(None, vec![part(text, Tone::Plain)])
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
        chips.push(plain(untracked_words(untracked.0)));
    }
    let python = match (&facts.venv, &facts.python) {
        (Some(venv), _) => Some(venv.as_str()),
        (None, Some(version)) => Some(version.as_str()),
        (None, None) => None,
    };
    if let Some(python) = python {
        chips.push(plain(format!("python {python}")));
    }
    if let Some(ruby) = &facts.ruby {
        chips.push(plain(format!("ruby {}", ruby.as_str())));
    }
    if let Some(go) = &facts.go {
        chips.push(plain(format!("go {}", go.as_str())));
    }
    if let Some(node) = &facts.node {
        chips.push(plain(format!("node {}", node.as_str())));
    }
    chips
}

/// Whether the chips stand over the header line: it is a line of its own and starts with the
/// folder as temor's prompt writes it (the person's own prompt, or a folder the shell names
/// differently, keeps its text). `folder` is None on another computer.
pub fn chips_go_over(header: &PromptHeader, folder: Option<&str>) -> bool {
    let Some(folder) = folder.filter(|f| !f.is_empty()) else {
        return false;
    };
    header.line == HeaderLine::Own
        && header
            .text
            .strip_prefix(folder)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// The folder and branch chips that lead the row over temor's header line.
pub fn lead_chips(folder: &str, facts: &Facts) -> Vec<ChipText> {
    let mut chips = vec![chip(
        Some(ChipIcon::Folder),
        vec![part(folder, Tone::Strong)],
    )];
    if let Some(branch) = &facts.branch {
        chips.push(chip(
            Some(ChipIcon::Branch),
            vec![part(branch.as_str(), Tone::Strong)],
        ));
    }
    chips
}

/// The chips and their place for a session: on the header line of the prompt being typed at,
/// when it is on screen. `folder` is the pane's folder as its chip says it, None on another
/// computer. `None` when there is nothing to say, on a full-screen program's screen, or before
/// the first layout.
pub fn chips_for(session: &Session, facts: &Facts, folder: Option<&str>) -> Option<ChipsView> {
    let gutter = session.gutter_geometry()?;
    let (inset, cell_h) = (gutter.side, gutter.cell_h);
    // The prompt being typed at, when it is in view and not on the alternate screen.
    let header = session.live_prompt_header()?;
    let top = gutter.row_top(header.row);
    let view = |chips: Vec<ChipText>, place| {
        (!chips.is_empty()).then_some(ChipsView {
            chips,
            place,
            top,
            height: cell_h,
            inset,
        })
    };
    if let Some(folder) = folder.filter(|_| chips_go_over(&header, folder)) {
        let mut chips = lead_chips(folder, facts);
        chips.extend(chip_texts(facts));
        return view(chips, ChipPlace::Over);
    }
    let mut chips = chip_texts(facts);
    // A command typed on this very row runs under the chips: leave out the ones it reaches.
    let (pane_width, cell_width) = session.grid_logical()?;
    let cursor_right = session
        .cursor_logical()
        .filter(|[_, y, _, h]| (y - top).abs() < h / 2.0)
        .map(|[x, _, w, _]| x + w);
    // A chip's picture takes about two cells.
    let words: Vec<usize> = chips
        .iter()
        .map(|c| c.text().chars().count() + 2 * usize::from(c.icon.is_some()))
        .collect();
    let geometry = ChipRow {
        pane_width,
        inset,
        cell_width,
    };
    chips.truncate(chips_that_fit(&words, geometry, cursor_right));
    view(chips, ChipPlace::End)
}

// The chips' pictures: Lucide's `folder`, `git-branch` and `diff` (lucide.dev, ISC licence,
// Copyright (c) Lucide Contributors; see NOTICE), as masks painted in the chip's text colour.
const FOLDER_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"##;
const BRANCH_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>"##;
const CHANGES_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v14"/><path d="M5 10h14"/><path d="M5 21h14"/></svg>"##;

const ICON_SIZE: IconSize = IconSize::Tiny;

fn icon_source(icon: ChipIcon) -> IconSource {
    let svg = match icon {
        ChipIcon::Folder => FOLDER_SVG,
        ChipIcon::Branch => BRANCH_SVG,
        ChipIcon::Changes => CHANGES_SVG,
    };
    IconSource::Symbolic(ExternalIcon {
        url: IconUrl::svg(svg),
        size: ICON_SIZE,
    })
}

fn icon_slug(icon: Option<ChipIcon>) -> &'static str {
    match icon {
        Some(ChipIcon::Folder) => "folder",
        Some(ChipIcon::Branch) => "branch",
        Some(ChipIcon::Changes) => "changes",
        None => "fact",
    }
}

/// The chips over the prompt's header row.
#[component]
#[allow(non_snake_case)]
pub fn FactChips(view: ChipsView) -> Element {
    let (place, pad) = match view.place {
        ChipPlace::Over => ("over", "left"),
        ChipPlace::End => ("end", "right"),
    };
    rsx! {
        div {
            class: "temor-fact-chips",
            "data-place": place,
            style: "top:{view.top}px; height:{view.height}px; padding-{pad}:{view.inset}px",
            for (n, one) in view.chips.iter().enumerate() {
                span {
                    key: "{n}",
                    class: "temor-chip",
                    "data-kind": icon_slug(one.icon),
                    if let Some(icon) = one.icon {
                        IconView { source: icon_source(icon), size: ICON_SIZE }
                    }
                    span { class: "temor-chip-words",
                        for (m, run) in one.parts.iter().enumerate() {
                            span { key: "{m}", "data-tone": run.tone.slug(), "{run.text}" }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "fact_chips_tests.rs"]
mod tests;
