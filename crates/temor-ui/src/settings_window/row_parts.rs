//! The parts the Settings window's rows share: a button named for the row it acts on, and a text
//! field that keeps its edit when the caret leaves.

use dioxus::prelude::*;
use ds::prelude::*;

/// A button named for what it does to whom, for assistive technology and tests.
pub fn named(name: impl Into<String>) -> Common {
    Common {
        aria_label: Some(name.into()),
        ..Common::default()
    }
}

/// "Remove" on a row. `what` names the row for assistive technology ("Remove {what}"), so a list
/// of several rows has a button for each that says which it takes away; `tip` is the hover hint.
#[component]
#[allow(non_snake_case)]
pub fn RemoveButton(
    #[props(into)] what: String,
    tip: &'static str,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        Button {
            label: "Remove".to_owned(),
            title: Some(tip.to_owned()),
            common: named(format!("Remove {what}")),
            onclick: move |_| onclick.call(()),
        }
    }
}

/// "Add" on the row that makes a new entry. `what` names the entry ("Add {what}"); `tip` is the
/// hover hint.
#[component]
#[allow(non_snake_case)]
pub fn AddButton(
    #[props(into)] what: String,
    tip: &'static str,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        Button {
            label: "Add".to_owned(),
            title: Some(tip.to_owned()),
            common: named(format!("Add {what}")),
            onclick: move |_| onclick.call(()),
        }
    }
}

/// A line of text that follows the keys as they are typed and calls `onchange` with the text when
/// the caret leaves or Return is pressed, if it differs from the `value` the row was drawn with.
/// `oninput` hears each edit, for a row that reacts to what is being typed.
#[component]
#[allow(non_snake_case)]
pub fn KeptField(
    #[props(into)] label: String,
    value: String,
    #[props(into, default)] placeholder: String,
    #[props(default)] oninput: EventHandler<String>,
    onchange: EventHandler<String>,
) -> Element {
    let mut typed = use_signal({
        let value = value.clone();
        move || value
    });
    rsx! {
        TextField {
            label,
            value: typed(),
            placeholder,
            oninput: move |text: String| {
                typed.set(text.clone());
                oninput.call(text);
            },
            onchange: move |text: String| {
                if text != value {
                    onchange.call(text);
                }
            },
        }
    }
}
