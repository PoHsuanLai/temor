//! The text rows of a Space's section: a line of text kept when the caret leaves, a route with its
//! warning, and the row that adds a route.

use dioxus::prelude::*;
use ds::components::fields::field_row::FieldRow;
use ds::prelude::*;

use super::space_edit::route_warning;

/// A row of one line of text, kept when the caret leaves or Return is pressed.
#[component]
#[allow(non_snake_case)]
pub fn SpaceText(
    #[props(into)] label: String,
    #[props(into)] help: String,
    #[props(into)] placeholder: String,
    warning: Option<&'static str>,
    value: String,
    onchange: EventHandler<String>,
) -> Element {
    let mut typed = use_signal({
        let value = value.clone();
        move || value
    });
    let note = warning.map_or(help.clone(), |warning| format!("{warning} {help}"));
    rsx! {
        FieldRow { label: TextLine::from(label.clone()), help: Some(TextLine::from(note)),
            TextField {
                label,
                value: typed(),
                placeholder,
                oninput: move |text: String| typed.set(text),
                onchange: move |text: String| {
                    if text != value {
                        onchange.call(text);
                    }
                },
            }
        }
    }
}

/// One route: its pattern, a warning when it is empty or matches everything, and Remove.
#[component]
#[allow(non_snake_case)]
pub fn RouteRow(
    pattern: String,
    onedit: EventHandler<String>,
    onremove: EventHandler<()>,
) -> Element {
    let mut typed = use_signal({
        let pattern = pattern.clone();
        move || pattern
    });
    let stored = pattern;
    let help =
        route_warning(&typed.read()).unwrap_or("New sessions that match open in this Space.");
    rsx! {
        FieldRow { label: "Route", help: Some(TextLine::from(help)),
            TextField {
                label: "Route".to_owned(),
                value: typed(),
                placeholder: "ssh prod-* or ~/work".to_owned(),
                oninput: move |text: String| typed.set(text),
                onchange: move |text: String| {
                    if text != stored {
                        onedit.call(text);
                    }
                },
            }
            Button {
                label: "Remove".to_owned(),
                title: Some("Take this route away".to_owned()),
                onclick: move |_| onremove.call(()),
            }
        }
    }
}

/// The row that adds a route.
#[component]
#[allow(non_snake_case)]
pub fn AddRoute(onadd: EventHandler<String>) -> Element {
    let mut draft = use_signal(String::new);
    let add = Callback::new(move |text: String| {
        draft.set(String::new());
        onadd.call(text);
    });
    rsx! {
        FieldRow {
            label: "New route",
            help: Some(TextLine::from("A command such as ssh prod-*, or a folder such as ~/work.")),
            TextField {
                label: "New route".to_owned(),
                value: draft(),
                placeholder: "ssh prod-* or ~/work".to_owned(),
                oninput: move |text: String| draft.set(text),
                onsubmit: move |text: String| add.call(text),
            }
            Button {
                label: "Add".to_owned(),
                title: Some("Add this route".to_owned()),
                onclick: move |_| add.call(draft()),
            }
        }
    }
}
