//! What a pane shows under its terminal (PLAN §6): the host banners, the offer, the status line,
//! the paste question and the "shell has ended" banner.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::prelude::*;
use temor_core::vocab::ids::PaneId;
use temor_pane::paste::UnsafeReason;

use super::blocks::Chrome;
use super::confirm::PasteConfirm;
use super::handlers::{paste_cancel, paste_confirm, restart_pane};
use super::host_banners::HostBanners;
use super::offer::OfferBar;
use super::slot::Slot;
use super::status::StatusLine;
use crate::hub::Desk;

/// Whether the pane's shell is still there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Running,
    Ended,
}

/// The pane's lower part. `pending` is the paste waiting for an answer.
pub fn pane_footer(
    pane: PaneId,
    desk: &Desk,
    slot: &Rc<RefCell<Slot>>,
    ui: Signal<u32>,
    chrome: &Chrome,
    pending: Option<UnsafeReason>,
    shell: Shell,
) -> Element {
    rsx! {
        HostBanners { pane, desk: desk.clone() }
        OfferBar { pane, desk: desk.clone() }
        StatusLine {
            cwd: chrome.cwd.clone(),
            title: chrome.title.clone(),
            progress: chrome.progress,
            bell: chrome.bell,
            link: chrome.link.clone(),
        }
        if let Some(reason) = pending {
            PasteConfirm {
                reason,
                on_paste:paste_confirm(Rc::clone(slot), ui),
                on_cancel:paste_cancel(Rc::clone(slot), ui),
            }
        }
        if shell == Shell::Ended {
            InlineBanner {
                severity: Severity::Warn,
                text: "The shell has ended.".to_owned(),
                actions: rsx! {
                    Button {
                        label: "Restart".to_owned(),
                        title: Some("Start a new shell".to_owned()),
                        onclick:restart_pane(desk.clone(), pane),
                    }
                },
            }
        }
    }
}
