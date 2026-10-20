//! The text rows of a Space's section: a line of text kept when the caret leaves, a route with its
//! warning, and the row that adds a route.

use dioxus::prelude::*;
use ds::components::fields::field_row::FieldRow;
use ds::prelude::*;

use super::row_parts::{AddButton, KeptField, RemoveButton};
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
    let note = warning.map_or(help.clone(), |warning| format!("{warning} {help}"));
    rsx! {
        FieldRow { label: TextLine::from(label.clone()), help: Some(TextLine::from(note)),
            KeptField { label, value, placeholder, onchange }
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
    let help =
        route_warning(&typed.read()).unwrap_or("New sessions that match open in this Space.");
    rsx! {
        FieldRow { label: "Route", help: Some(TextLine::from(help)),
            KeptField {
                label: "Route",
                value: pattern.clone(),
                placeholder: "ssh prod-* or ~/work",
                oninput: move |text: String| typed.set(text),
                onchange: onedit,
            }
            RemoveButton {
                what: format!("route {pattern}"),
                tip: "Take this route away",
                onclick: onremove,
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
            AddButton {
                what: "route",
                tip: "Add this route",
                onclick: move |()| add.call(draft()),
            }
        }
    }
}
