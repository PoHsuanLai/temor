//! The question when a program wants the clipboard (PLAN §6): to copy to it, or to read it.
//! The answer is kept for the tab.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::components::overlays::alert_model::{AlertButton, AlertRole};
use ds::prelude::*;
use temor_core::remote::Grant;

use super::slot::Slot;
use crate::session::{ClipboardAsk, ClipboardUse};

/// The title and message for `ask`, in the person's words.
pub fn ask_words(ask: &ClipboardAsk) -> (&'static str, String) {
    let who = match ask.program.is_empty() {
        true => "A program".to_owned(),
        false => ask.program.clone(),
    };
    match ask.kind {
        ClipboardUse::Copy => (
            "Allow copying?",
            format!("{who} wants to put text on your clipboard. Your answer is kept for this tab."),
        ),
        ClipboardUse::Read => (
            "Allow reading the clipboard?",
            format!(
                "{who} wants to read what you last copied. If you allow it, it sees your \
                 clipboard the next time it asks. Your answer is kept for this tab."
            ),
        ),
    }
}

/// Asks whether a program may use the clipboard.
#[component]
#[allow(non_snake_case)]
pub fn ClipboardConfirm(
    ask: ClipboardAsk,
    on_allow: EventHandler<()>,
    on_refuse: EventHandler<()>,
) -> Element {
    let (title, message) = ask_words(&ask);
    rsx! {
        Alert {
            title,
            message: Some(message.into()),
            buttons: vec![
                AlertButton {
                    label: "Allow".to_owned(),
                    role: AlertRole::Normal,
                    onpress: Callback::new(move |_| on_allow.call(())),
                    common: Common::default(),
                },
                AlertButton {
                    label: "Don't Allow".to_owned(),
                    role: AlertRole::Cancel,
                    onpress: Callback::new(move |_| on_refuse.call(())),
                    common: Common::default(),
                },
            ],
        }
    }
}

/// The question's answer, then a redraw.
pub fn clipboard_answer(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
    grant: Grant,
    keep: impl Fn(Grant) + 'static,
) -> impl FnMut(()) + 'static {
    move |_: ()| {
        let copy = slot
            .borrow()
            .session
            .as_ref()
            .and_then(|s| s.clipboard_ask().map(|a| a.kind))
            == Some(ClipboardUse::Copy);
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.answer_clipboard(grant);
        }
        // A write's answer is kept per host; a read's stays with the tab.
        if copy {
            keep(grant);
        }
        ui += 1;
    }
}
