//! The context chips (PLAN P16), as Warp draws them: a row of small rounded chips on the
//! prompt's header line saying what the pane's folder is: the folder, the git branch, the lines
//! added and removed and the files changed, new files, the Python environment and the tool
//! versions. With temor's own prompt the row stands over the header line, left to right, in place
//! of the folder and branch the shell wrote there. With a one-line prompt, or a header that says
//! something else (the person's own prompt, another computer's folder), the shell's text stays and
//! the facts' chips end at the row's right edge instead, without the folder and branch.
//!
//! What the chips say is kept for the block the prompt becomes (`Session::keep_header_facts`):
//! its card names the folder.
//!
//! They sit beside the grid, so a press never reaches the terminal, and they are not drawn on a
//! full-screen program's screen.

use dioxus::prelude::*;
use ds::prelude::{Icon, IconSize, IconSource, IconView};
use temor_core::facts::{CHIP_FRAME, ChipRow, Facts, chips_that_fit};

pub use super::chip_texts::{all_chips, chip_texts, lead_chips, plugin_chips};
use super::folder_fit::{RowRoom, fit_folder_chip};
use crate::plugins::facts::PluginFact;
use crate::session::{HeaderLine, PromptHeader, Session};

/// The small picture at a chip's start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipIcon {
    Folder,
    Branch,
    Changes,
}

/// What a chip says; each kind has its own colour from the terminal's palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    Folder,
    Branch,
    Changes,
    New,
    Python,
    Ruby,
    Go,
    Node,
    /// A fact a plugin gave.
    Plugin,
}

impl ChipKind {
    fn slug(self) -> &'static str {
        match self {
            ChipKind::Folder => "folder",
            ChipKind::Branch => "branch",
            ChipKind::Changes => "changes",
            ChipKind::New => "new",
            ChipKind::Python => "python",
            ChipKind::Ruby => "ruby",
            ChipKind::Go => "go",
            ChipKind::Node => "node",
            ChipKind::Plugin => "plugin",
        }
    }
}

/// How a run of a chip's words is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// The chip's own colour.
    Plain,
    /// What the chip is named for (the folder, the branch, the tool), in the chip's colour.
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

/// One chip: what it says, its picture, if any, and its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipText {
    pub kind: ChipKind,
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

/// The chips and their place for a session: on the header line of the prompt being typed at,
/// when it is on screen. `folder` is the pane's folder as its chip says it, None on another
/// computer. `None` when there is nothing to say, on a full-screen program's screen, or before
/// the first layout.
pub fn chips_for(
    session: &Session,
    facts: &Facts,
    plugin: &[PluginFact],
    folder: Option<&str>,
) -> Option<ChipsView> {
    let gutter = session.gutter_geometry()?;
    let (inset, cell_h) = (gutter.side, gutter.cell_h);
    // The prompt being typed at, when it is in view and not on the alternate screen.
    let header = session.frame()?.live_header()?;
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
        chips.extend(all_chips(facts, plugin));
        // A long folder keeps its end: the row is the pane's width less the padding.
        if let Some((pane_width, cell_width)) = session.grid_logical() {
            let row = RowRoom {
                width: pane_width - inset,
                cell_width,
            };
            fit_folder_chip(&mut chips, row);
        }
        return view(chips, ChipPlace::Over);
    }
    let mut chips = all_chips(facts, plugin);
    // A command typed on this very row runs under the chips: leave out the ones it reaches.
    let (pane_width, cell_width) = session.grid_logical()?;
    let cursor_right = session
        .cursor_logical()
        .filter(|[_, y, _, h]| (y - top).abs() < h / 2.0)
        .map(|[x, _, w, _]| x + w);
    // A chip's picture takes about as many cells as its frame.
    let words: Vec<usize> = chips
        .iter()
        .map(|c| c.text().chars().count() + CHIP_FRAME as usize * usize::from(c.icon.is_some()))
        .collect();
    let geometry = ChipRow {
        pane_width,
        inset,
        cell_width,
    };
    chips.truncate(chips_that_fit(&words, geometry, cursor_right));
    view(chips, ChipPlace::End)
}

const ICON_SIZE: IconSize = IconSize::Tiny;

/// The chips' pictures: quire's outline folder, git branch and diff (plus over minus) glyphs,
/// stroked in the chip's text colour.
fn icon_source(icon: ChipIcon) -> IconSource {
    IconSource::Glyph(match icon {
        ChipIcon::Folder => Icon::Folder,
        ChipIcon::Branch => Icon::GitBranch,
        ChipIcon::Changes => Icon::Diff,
    })
}

/// The chips over the prompt's header row.
#[component]
#[allow(non_snake_case)]
pub fn FactChips(view: ChipsView) -> Element {
    let place = match view.place {
        ChipPlace::Over => "over",
        ChipPlace::End => "end",
    };
    let at = format!("top:{}px; height:{}px", view.top, view.height);
    let style = match view.place {
        ChipPlace::Over => format!("{at}; padding-left:{}px", view.inset),
        ChipPlace::End => format!("{at}; padding-right:{}px", view.inset),
    };
    rsx! {
        div {
            class: "temor-fact-chips",
            "data-place": place,
            style,
            for (n, one) in view.chips.iter().enumerate() {
                span {
                    key: "{n}",
                    class: "temor-chip",
                    "data-kind": one.kind.slug(),
                    if let Some(icon) = one.icon {
                        span { class: "temor-chip-icon",
                            IconView { source: icon_source(icon), size: ICON_SIZE }
                        }
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
