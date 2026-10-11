//! The first-launch hint (PLAN §7): one quiet line under the prompt of a new window's first
//! pane, naming the chords for the command bar and for a split, until the first command runs.
//! Restored sessions don't show it. It sits beside the grid, so a press never reaches the
//! terminal.

use dioxus::prelude::*;
use ds::components::content::label::{Label, LabelRole, LabelStyle};

use crate::session::Session;

/// Where the hint goes and what it says.
#[derive(Debug, Clone, PartialEq)]
pub struct HintView {
    pub text: String,
    /// The line's top in logical pixels.
    pub top: f32,
    /// The line's left edge in logical pixels.
    pub left: f32,
}

/// The hint and its place for a session: one row under the cursor, while no command has run.
/// `None` after the first command, on a full-screen program's screen, or before the first
/// layout.
pub fn hint_for(session: &Session, text: String) -> Option<HintView> {
    if !session.block_outcomes().is_empty() {
        return None;
    }
    let (inset, _) = session.gutter_geometry()?;
    let [_, y, _, height] = session.cursor_logical()?;
    Some(HintView {
        text,
        top: y + height,
        left: inset,
    })
}

/// The hint line.
#[component]
#[allow(non_snake_case)]
pub fn FirstHint(view: HintView) -> Element {
    rsx! {
        div {
            class: "temor-first-hint",
            style: "top:{view.top}px; left:{view.left}px",
            Label {
                text: view.text.clone(),
                role: LabelRole::Tertiary,
                style: LabelStyle::Footnote,
            }
        }
    }
}
