//! The block gutter and the block bar (PLAN §9 blocks). Each block's prompt row gets a small
//! exit mark in the pane's left margin; hovering it shows how the command ended. A gutter click
//! selects the block, and the bar under the pane acts on it. The marks never take pointer events:
//! the pane's own element hears the pointer (quire v0.2.30 drops clicks on custom widgets).

use dioxus::prelude::*;
use ds::components::content::label::LabelStyle;
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::overlays::tooltip::Tooltip;
use ds::prelude::*;
use temor_core::block::Outcome;
use temor_core::vocab::ids::BlockId;

use super::status::{ProgressLine, progress_line};
use crate::session::Session;

/// One exit mark, placed by the window from the session's marks.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkView {
    pub id: BlockId,
    /// Distance from the top of the pane, in logical pixels.
    pub top: f32,
    /// Failure asks for attention; success is quiet.
    pub failed: bool,
    /// What the hover says.
    pub summary: String,
    pub hovered: bool,
}

/// The exit marks in the gutter.
#[component]
#[allow(non_snake_case)]
pub fn GutterMarks(marks: Vec<MarkView>) -> Element {
    rsx! {
        for mark in marks {
            Tooltip {
                text: mark.summary.clone(),
                shown: Some(match mark.hovered {
                    true => Shown::Visible,
                    false => Shown::Hidden,
                }),
                div { style: "position:absolute; left:2px; top:{mark.top}px; pointer-events:none",
                    Badge {
                        content: BadgeContent::Dot,
                        tone: match mark.failed {
                            true => BadgeTone::Alert,
                            false => BadgeTone::Quiet,
                        },
                    }
                }
            }
        }
    }
}

/// What the block bar shows and does for the selected block.
#[derive(Debug, Clone, PartialEq)]
pub struct BarView {
    pub id: BlockId,
    /// How it ended, in words.
    pub summary: String,
    /// Whether its output is complete (copy and save need it).
    pub finished: bool,
    /// Whether it can be typed again (it has a command).
    pub can_rerun: bool,
}

/// The bar under the pane for the selected block.
/// The actions for the selected block: each reports the block it acts on.
#[component]
#[allow(non_snake_case)]
pub fn BlockBar(
    bar: BarView,
    on_copy_command: EventHandler<BlockId>,
    on_copy_output: EventHandler<BlockId>,
    on_save: EventHandler<BlockId>,
    on_rerun: EventHandler<BlockId>,
    on_close: EventHandler<()>,
) -> Element {
    let id = bar.id;
    rsx! {
        div { style: "display:flex; align-items:center; overflow:hidden; white-space:nowrap; flex:0 0 auto",
            Label { text: bar.summary.clone(), style: LabelStyle::Caption }
            Button {
                label: "Copy command".to_owned(),
                title: Some("Copy this command".to_owned()),
                onclick: move |_| on_copy_command.call(id),
            }
            Button {
                label: "Copy output".to_owned(),
                title: Some("Copy this command's output".to_owned()),
                availability: match bar.finished {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
                onclick: move |_| on_copy_output.call(id),
            }
            Button {
                label: "Save output".to_owned(),
                title: Some("Save this command's output to Downloads".to_owned()),
                availability: match bar.finished {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
                onclick: move |_| on_save.call(id),
            }
            Button {
                label: "Re-run".to_owned(),
                title: Some("Put this command in the prompt. Press Return to run it".to_owned()),
                availability: match bar.can_rerun {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
                onclick: move |_| on_rerun.call(id),
            }
            Button {
                label: "Done".to_owned(),
                title: Some("Hide these actions".to_owned()),
                onclick: move |_| on_close.call(()),
            }
        }
    }
}

/// What the window draws beside the pane, read from the session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chrome {
    pub marks: Vec<MarkView>,
    pub bar: Option<BarView>,
    pub cwd: Option<String>,
    pub title: String,
    pub progress: Option<ProgressLine>,
    pub bell: bool,
}

/// The chrome for a session, with `hovered` the block whose mark the pointer is over.
pub fn chrome_of(session: &Session, hovered: Option<BlockId>) -> Chrome {
    let (pad, cell_h) = session.gutter_geometry().unwrap_or((0.0, 0.0));
    let marks = session
        .block_marks()
        .into_iter()
        .map(|mark| MarkView {
            id: mark.id,
            // Centred on the prompt's row.
            top: pad + mark.row as f32 * cell_h + (cell_h / 2.0 - 3.0).max(0.0),
            failed: matches!(mark.outcome, Outcome::Failed(_)),
            summary: session.block_summary(mark.id).unwrap_or_default(),
            hovered: hovered == Some(mark.id),
        })
        .collect();
    let bar = session.selected_block().map(|id| BarView {
        id,
        summary: session.block_summary(id).unwrap_or_default(),
        finished: session.block_finished(id),
        can_rerun: session.block_command(id).is_some_and(|c| !c.is_empty()),
    });
    Chrome {
        marks,
        bar,
        cwd: session.cwd().map(str::to_owned),
        title: session.title().to_owned(),
        progress: session.progress().and_then(progress_line),
        bell: session.bell_showing(),
    }
}
