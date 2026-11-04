//! The question when a program wants the clipboard (PLAN §6): to copy to it, or to read it.
//! The answer is kept for the tab.

use dioxus::prelude::*;
use temor_client::model::remote::Grant;

use super::confirm::two_button_alert;
use super::pane_link::{PaneLink, Redraw};
use temor_pane_view::session::{ClipboardAsk, ClipboardUse};

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
    two_button_alert(
        title,
        message,
        ("Allow", on_allow),
        ("Don't Allow", on_refuse),
    )
}

/// The question's answer, then a redraw.
pub fn clipboard_answer(
    link: PaneLink,
    grant: Grant,
    keep: impl Fn(Grant) + 'static,
) -> impl FnMut(()) + 'static {
    move |_: ()| {
        let asked = link.send(Redraw::WindowOnly, |s| {
            let asked = s.clipboard_ask().map(|a| a.kind);
            s.answer_clipboard(grant);
            asked
        });
        // A write's answer is kept per host; a read's stays with the tab.
        if asked.flatten() == Some(ClipboardUse::Copy) {
            keep(grant);
        }
    }
}
