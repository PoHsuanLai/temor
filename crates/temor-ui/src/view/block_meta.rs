//! How a command card ended, at the right end of its header row (PLAN P15): "exit 1 · 4 ms" when
//! it failed, "4 ms" when it ran, nothing while it runs. It is read from the block model (the
//! outcome and the times the session measured), never from the shell's own text.
//!
//! It is a positioned element beside the grid, like the context chips, and not a renderer band:
//! bands only tint and stripe rows and cannot set text in quire's type at the secondary ink, and
//! the hover tools (`block_tools`) share the row with it. It is not drawn on a full-screen
//! program's screen.

use dioxus::prelude::*;
use ds::components::content::label::{LabelRole, LabelStyle};
use ds::prelude::*;
use temor_core::block::{Outcome, format_elapsed};
use temor_core::vocab::ids::BlockId;

use crate::session::{BlockMark, Gutter, Session};

/// One header row's meta, placed by the window from the session.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaView {
    pub id: BlockId,
    pub text: String,
    /// The header row's top and height in logical pixels.
    pub top: f32,
    pub height: f32,
    /// The pane's padding in logical pixels: the text ends this far from the right edge.
    pub inset: f32,
}

/// The words for a block that ended: `exit N · T` for a failure, `T` otherwise. `None` while it
/// runs, and when no time was measured for one that did not fail.
pub fn meta_text(outcome: Outcome, took_ms: Option<u64>) -> Option<String> {
    let took = took_ms.map(format_elapsed);
    match (outcome, took) {
        (Outcome::Running, _) => None,
        (Outcome::Failed(code), Some(took)) => Some(format!("exit {} \u{b7} {took}", code.0)),
        (Outcome::Failed(code), None) => Some(format!("exit {}", code.0)),
        (_, took) => took,
    }
}

/// The header rows' meta for the blocks whose prompts are on screen, except `skip` (the block
/// whose tools show the meta themselves) and those with no room right of their header's text.
pub fn metas_of(session: &Session, skip: Option<BlockId>) -> Vec<MetaView> {
    let Some(gutter) = session.gutter_geometry() else {
        return Vec::new();
    };
    let row = header_row(session, &gutter);
    session
        .block_marks()
        .into_iter()
        .filter(|mark| Some(mark.id) != skip)
        .filter_map(|mark| {
            meta_of(&mark, &gutter)
                .filter(|meta| row.is_some_and(|row| row.fits(mark.header_cols, &meta.text)))
        })
        .collect()
}

/// The room on a header row, once the pane has a size.
pub fn header_row(session: &Session, gutter: &Gutter) -> Option<HeaderRow> {
    session
        .grid_logical()
        .map(|(pane_width, cell_width)| HeaderRow {
            pane_width,
            cell_width,
            inset: gutter.side,
        })
}

/// A header row's room, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaderRow {
    pub pane_width: f32,
    pub cell_width: f32,
    /// The pane's padding on each side.
    pub inset: f32,
}

impl HeaderRow {
    /// Whether `words`, set at the row's right end, clear a header text `cols` columns long with
    /// a column to spare. Each character is counted a terminal cell wide: the meta's type is
    /// smaller, so what fits here fits on screen.
    pub fn fits(self, cols: u32, words: &str) -> bool {
        let used = cols as f32 + 1.0 + words.chars().count() as f32;
        self.inset + used * self.cell_width <= self.pane_width - self.inset
    }
}

/// On the header line, beside the folder and branch.
fn meta_of(mark: &BlockMark, gutter: &Gutter) -> Option<MetaView> {
    Some(MetaView {
        id: mark.id,
        text: meta_text(mark.outcome, mark.took_ms)?,
        top: gutter.row_top(mark.header),
        height: gutter.cell_h,
        inset: gutter.side,
    })
}

/// The meta text itself: secondary ink, in the terminal's own face (it is shell data).
#[component]
#[allow(non_snake_case)]
pub fn MetaText(text: String) -> Element {
    rsx! {
        span { class: "ds-mono",
            Label { text, role: LabelRole::Secondary, style: LabelStyle::Footnote }
        }
    }
}

/// The meta of every header row on screen.
#[component]
#[allow(non_snake_case)]
pub fn BlockMetas(metas: Vec<MetaView>) -> Element {
    rsx! {
        for meta in metas {
            div {
                key: "{meta.id.0}",
                class: "temor-card-meta",
                style: "top:{meta.top}px; height:{meta.height}px; right:{meta.inset}px",
                MetaText { text: meta.text.clone() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_core::vocab::ids::ExitCode;

    #[test]
    fn the_meta_shows_only_where_it_clears_the_header_text() {
        // An 80-column pane: 8 px cells, 8 px insets, 656 px wide.
        let row = HeaderRow {
            pane_width: 656.0,
            cell_width: 8.0,
            inset: 8.0,
        };
        // (what, header columns, words, fits)
        for (what, cols, words, fits) in [
            ("a short folder", 12, "exit 1 \u{b7} 4 ms", true),
            ("exactly to the edge", 66, "exit 1 · 4 ms", true),
            ("one column too long", 67, "exit 1 · 4 ms", false),
            ("a folder across the row", 80, "4 ms", false),
            ("an empty header", 0, "4 ms", true),
        ] {
            assert_eq!(row.fits(cols, words), fits, "{what}");
        }
    }

    #[test]
    fn the_meta_says_how_it_ended_and_nothing_while_it_runs() {
        let failed = Outcome::Failed(ExitCode(1));
        // (what, outcome, took ms, words)
        for (what, outcome, took, want) in [
            ("failed", failed, Some(4), Some("exit 1 \u{b7} 4 ms")),
            ("failed, no time", failed, None, Some("exit 1")),
            ("ran", Outcome::Succeeded, Some(4_200), Some("4.2 s")),
            (
                "stopped is not a failure",
                Outcome::Stopped(ExitCode(130)),
                Some(900),
                Some("900 ms"),
            ),
            (
                "ended unreported",
                Outcome::Unreported,
                Some(70_000),
                Some("1 min 10 s"),
            ),
            ("no time", Outcome::Succeeded, None, None),
            ("running", Outcome::Running, None, None),
        ] {
            assert_eq!(meta_text(outcome, took).as_deref(), want, "{what}");
        }
    }
}
