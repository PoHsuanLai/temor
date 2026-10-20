//! The question a program puts to the person (PLAN §9): it wants to save files to Downloads, or
//! to be given one. Shown above the panes until answered; nothing is saved or sent before the
//! answer. An "Allow" is remembered for the same program stream until temor quits.

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_transfer::Consent;

use crate::hub::Desk;

/// The question in front, if there is one.
#[component]
#[allow(non_snake_case)]
pub fn TransferAsk(desk: Desk) -> Element {
    let _ = (desk.ui)();
    let Some(ask) = desk.hub.front_ask() else {
        return rsx! {};
    };
    let view = ask.view();
    let allow = {
        let desk = desk.clone();
        move |_: Press| answer(&desk, Consent::Allow)
    };
    let refuse = {
        let desk = desk.clone();
        move |_: Press| answer(&desk, Consent::Refuse)
    };
    rsx! {
        InlineBanner {
            severity: Severity::Info,
            text: view.text,
            detail: Some(TextLine::from(view.detail)),
            actions: rsx! {
                Button {
                    label: "Allow".to_owned(),
                    size: ControlSize::Small,
                    title: Some("Let the program do this".to_owned()),
                    onclick: allow,
                }
                Button {
                    label: "Don't allow".to_owned(),
                    size: ControlSize::Small,
                    title: Some("Refuse, and don't ask again for this program".to_owned()),
                    onclick: refuse,
                }
            },
        }
    }
}

fn answer(desk: &Desk, consent: Consent) {
    for text in desk.hub.answer_ask(consent) {
        desk.say(text);
    }
    desk.bump();
}
