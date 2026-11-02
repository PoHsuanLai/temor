//! What a pane shows under its terminal (PLAN §6): the host banners, the offer, the progress edge,
//! the paste question and the "shell has ended" banner.

use dioxus::prelude::*;
use ds::prelude::*;
use temor_panes::pane::paste::UnsafeReason;

use super::blocks::Chrome;
use super::confirm::PasteConfirm;
use super::handlers::{paste_cancel, paste_confirm, restart_pane};
use super::host_banners::HostBanners;
use super::offer::OfferBar;
use super::pane_link::PaneLink;
use super::status::PaneEdge;

/// Whether the pane's shell is still there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Running,
    Ended,
}

/// The pane's lower part. `pending` is the paste waiting for an answer.
pub fn pane_footer(
    link: &PaneLink,
    chrome: &Chrome,
    pending: Option<UnsafeReason>,
    shell: Shell,
) -> Element {
    let (pane, desk) = (link.pane, &link.desk);
    // A tab that runs a program in place of the shell names it, and Restart runs it again.
    let ended = match desk.hub.0.borrow().run_name(pane) {
        Some(name) => (format!("{name} has ended."), format!("Run {name} again")),
        None => (
            "The shell has ended.".to_owned(),
            "Start a new shell".to_owned(),
        ),
    };
    rsx! {
        HostBanners { pane, desk: desk.clone() }
        OfferBar { pane, desk: desk.clone() }
        PaneEdge { progress: chrome.progress, bell: chrome.bell }
        if let Some(reason) = pending {
            PasteConfirm {
                reason,
                on_paste:paste_confirm(link.clone()),
                on_cancel:paste_cancel(link.clone()),
            }
        }
        if shell == Shell::Ended {
            InlineBanner {
                severity: Severity::Warn,
                text: ended.0,
                actions: rsx! {
                    Button {
                        label: "Restart".to_owned(),
                        title: Some(ended.1),
                        onclick:restart_pane(desk.clone(), pane),
                    }
                },
            }
        }
    }
}
