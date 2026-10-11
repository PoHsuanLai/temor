//! The pane's bottom edge (PLAN §7): the program's progress and the bell's brief mark. There is no
//! status strip: the folder is in the card header and the tab's row, the title in the window's.

use dioxus::prelude::*;
use ds::components::content::label::LabelStyle;
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::controls::progress::model::{Progress as Share, ProgressStyle};
use ds::components::controls::progress::view::ProgressIndicator;
use ds::components::overlays::tooltip::Tooltip;
use ds::prelude::*;
use temor_vt::{Progress, ProgressState};

/// A progress report as the pane's edge shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressLine {
    /// A known share, 0 to 100.
    Share(u8),
    /// Busy with no amount known.
    Busy,
    /// The work reported an error.
    Failed,
}

/// What a progress report shows; `None` when it asks for the progress to be removed.
pub fn progress_line(progress: Progress) -> Option<ProgressLine> {
    match progress.state {
        ProgressState::Remove => None,
        ProgressState::Error => Some(ProgressLine::Failed),
        ProgressState::Indeterminate => Some(ProgressLine::Busy),
        ProgressState::Set | ProgressState::Pause => Some(ProgressLine::Share(
            progress.percent.map_or(0, |p| p.value()),
        )),
        // A state this build does not know: nothing is shown.
        _ => None,
    }
}

/// What the pane shows at its bottom edge: the program's progress and the bell's brief mark. The
/// folder and title live in the card header and the tab's row; a hovered link has its own tooltip
/// (`link_tip`). With nothing to show it draws nothing and takes no room.
#[component]
#[allow(non_snake_case)]
pub fn PaneEdge(progress: Option<ProgressLine>, bell: bool) -> Element {
    if progress.is_none() && !bell {
        return rsx! {};
    }
    rsx! {
        div { class: "temor-edge",
            if let Some(ProgressLine::Share(percent)) = progress {
                ProgressIndicator {
                    style: ProgressStyle::Bar,
                    progress: Share::Known(Fraction(u16::from(percent) * 10)),
                    common: Common { aria_label: Some("Progress".to_owned()), ..Common::default() },
                }
            }
            if progress == Some(ProgressLine::Busy) {
                Label { text: "Working", style: LabelStyle::Footnote }
            }
            if progress == Some(ProgressLine::Failed) {
                Label { text: "Failed", style: LabelStyle::Footnote }
            }
            if bell {
                Tooltip { text: "The bell rang", shown: None,
                    Badge { content: BadgeContent::Dot, tone: BadgeTone::Alert }
                }
            }
        }
    }
}
