//! The paste confirmation (PLAN §6): an unsafe paste is shown, and it goes to the program only
//! when the person says so.

use dioxus::prelude::*;
use ds::components::overlays::alert_model::{AlertButton, AlertRole};
use ds::prelude::*;
use temor_pane::paste::UnsafeReason;

/// Why a paste needs confirming, in the person's words.
pub fn reason_text(reason: UnsafeReason) -> &'static str {
    match reason {
        UnsafeReason::Newline => "It has a line break, so it would run as soon as it lands.",
        UnsafeReason::EndOfFrame => "It has a hidden marker that could end the paste early.",
        UnsafeReason::ControlCharacters => {
            "It has control characters, which would be sent as spaces."
        }
    }
}

/// Asks whether to paste text that is unsafe as it is.
#[component]
#[allow(non_snake_case)]
pub fn PasteConfirm(
    reason: UnsafeReason,
    on_paste: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    two_button_alert(
        "Paste this text?",
        reason_text(reason),
        ("Paste", on_paste),
        ("Cancel", on_cancel),
    )
}

/// A question with an answer to go on (`yes`) and one to turn back (`no`, the Cancel button), each
/// a label and what hears the press. The paste, the clipboard and the like all ask this way.
pub fn two_button_alert(
    title: &str,
    message: impl Into<TextLine>,
    (yes, on_yes): (&str, EventHandler<()>),
    (no, on_no): (&str, EventHandler<()>),
) -> Element {
    rsx! {
        Alert {
            title,
            message: Some(message.into()),
            buttons: vec![
                AlertButton {
                    label: yes.to_owned(),
                    role: AlertRole::Normal,
                    onpress: Callback::new(move |_| on_yes.call(())),
                    common: Common::default(),
                },
                AlertButton {
                    label: no.to_owned(),
                    role: AlertRole::Cancel,
                    onpress: Callback::new(move |_| on_no.call(())),
                    common: Common::default(),
                },
            ],
        }
    }
}
