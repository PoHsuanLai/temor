//! The status line under the pane (PLAN §7): the working directory the shell reported, the
//! program's title (the window's own title waits for quire v0.2.31), its progress and the bell's
//! brief mark.

use dioxus::prelude::*;
use ds::components::content::label::LabelStyle;
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::controls::progress::model::{Progress as Share, ProgressStyle};
use ds::components::controls::progress::view::ProgressIndicator;
use ds::components::overlays::tooltip::Tooltip;
use ds::prelude::*;
use temor_vt::{Progress, ProgressState};

/// A progress report as the status line shows it.
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
    }
}

/// The status line's content. The window's own title waits for quire v0.2.31 (a runtime window
/// title), so the program's title is shown here.
// TODO(quire 0.2.31): set the window title from the program's title (OSC 0/2) when quire can.
#[component]
#[allow(non_snake_case)]
pub fn StatusLine(
    cwd: Option<String>,
    title: String,
    progress: Option<ProgressLine>,
    bell: bool,
) -> Element {
    rsx! {
        div { style: "display:flex; align-items:center; overflow:hidden; white-space:nowrap; flex:0 0 auto",
            if let Some(cwd) = cwd {
                Label { text: cwd, style: LabelStyle::Caption }
            }
            if !title.is_empty() {
                Label { text: title, style: LabelStyle::Caption }
            }
            if let Some(ProgressLine::Share(percent)) = progress {
                ProgressIndicator {
                    style: ProgressStyle::Bar,
                    progress: Share::Known(Fraction(u16::from(percent) * 10)),
                    common: Common { aria_label: Some("Progress".to_owned()), ..Common::default() },
                }
            }
            if progress == Some(ProgressLine::Busy) {
                Label { text: "Working", style: LabelStyle::Caption }
            }
            if progress == Some(ProgressLine::Failed) {
                Label { text: "Failed", style: LabelStyle::Caption }
            }
            if bell {
                Tooltip { text: "The bell rang", shown: None,
                    Badge { content: BadgeContent::Dot, tone: BadgeTone::Alert }
                }
            }
        }
    }
}
