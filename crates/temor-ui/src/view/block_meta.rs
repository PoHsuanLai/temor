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

use crate::session::{BlockMark, Session};

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
/// whose tools show the meta themselves).
pub fn metas_of(session: &Session, skip: Option<BlockId>) -> Vec<MetaView> {
    let Some((inset, cell_h)) = session.gutter_geometry() else {
        return Vec::new();
    };
    session
        .block_marks()
        .into_iter()
        .filter(|mark| Some(mark.id) != skip)
        .filter_map(|mark| meta_of(&mark, inset, cell_h))
        .collect()
}

fn meta_of(mark: &BlockMark, inset: f32, cell_h: f32) -> Option<MetaView> {
    Some(MetaView {
        id: mark.id,
        text: meta_text(mark.outcome, mark.took_ms)?,
        top: inset + mark.row as f32 * cell_h,
        height: cell_h,
        inset,
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
