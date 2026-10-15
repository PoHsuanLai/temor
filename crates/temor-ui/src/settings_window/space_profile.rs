//! The rows of a Space's profile that hold variables and credentials. A variable is a name and a
//! value in plain fields. A credential shows only that it is set: its field is a secure one that
//! keeps the typed text out of the markup, it is emptied the moment the value is handed on, and
//! the stored value is never read back into the page.

use dioxus::prelude::*;
use ds::components::content::label::LabelRole;
use ds::components::fields::field_row::FieldRow;
use ds::components::fields::text_field_model::{FieldKind, FieldText};
use ds::prelude::*;
use ds::root::common::Common;

use crate::edge::Held;

/// A button named for what it does to whom, for assistive technology and tests.
fn named(name: String) -> Common {
    Common {
        aria_label: Some(name),
        ..Common::default()
    }
}

/// One variable: its name, its value, and Remove.
#[component]
#[allow(non_snake_case)]
pub fn VariableRow(
    name: String,
    value: String,
    onname: EventHandler<String>,
    onvalue: EventHandler<String>,
    onremove: EventHandler<()>,
) -> Element {
    let mut typed_name = use_signal({
        let name = name.clone();
        move || name
    });
    let mut typed_value = use_signal({
        let value = value.clone();
        move || value
    });
    let (stored_name, stored_value, subject) = (name.clone(), value, name.clone());
    rsx! {
        FieldRow {
            label: "Variable",
            help: Some(TextLine::from("Set in every new shell.")),
            TextField {
                label: format!("Name of {name}"),
                value: typed_name(),
                oninput: move |text: String| typed_name.set(text),
                onchange: move |text: String| {
                    if text != stored_name {
                        onname.call(text);
                    }
                },
            }
            TextField {
                label: format!("Value of {subject}"),
                value: typed_value(),
                oninput: move |text: String| typed_value.set(text),
                onchange: move |text: String| {
                    if text != stored_value {
                        onvalue.call(text);
                    }
                },
            }
            Button {
                label: "Remove".to_owned(),
                title: Some("Take this variable away".to_owned()),
                common: named(format!("Remove {name}")),
                onclick: move |_| onremove.call(()),
            }
        }
    }
}

/// The row that adds a variable.
#[component]
#[allow(non_snake_case)]
pub fn AddVariable(onadd: EventHandler<(String, String)>) -> Element {
    let mut name = use_signal(String::new);
    let mut value = use_signal(String::new);
    let add = Callback::new(move |(): ()| {
        let (n, v) = (name(), value());
        name.set(String::new());
        value.set(String::new());
        onadd.call((n, v));
    });
    rsx! {
        FieldRow {
            label: "New variable",
            help: Some(TextLine::from("A name, such as RUST_LOG, and its value.")),
            TextField {
                label: "New variable name".to_owned(),
                value: name(),
                oninput: move |text: String| name.set(text),
            }
            TextField {
                label: "New variable value".to_owned(),
                value: value(),
                oninput: move |text: String| value.set(text),
                onsubmit: move |_| add.call(()),
            }
            Button {
                label: "Add".to_owned(),
                title: Some("Add this variable".to_owned()),
                common: named("Add variable".to_owned()),
                onclick: move |_| add.call(()),
            }
        }
    }
}

/// One credential: whether its secret is set, a secure field for a new value, Replace (Set where
/// none is) and Remove. The stored value is not shown.
#[component]
#[allow(non_snake_case)]
pub fn CredentialRow(
    env: String,
    account: String,
    held: Held,
    onset: EventHandler<String>,
    onremove: EventHandler<()>,
) -> Element {
    let mut draft = use_signal(String::new);
    let (state, act) = match held {
        Held::Set => ("Set", "Replace"),
        Held::NotSet => ("Not set", "Set"),
    };
    let give = Callback::new(move |(): ()| {
        let secret = draft();
        draft.set(String::new());
        onset.call(secret);
    });
    rsx! {
        FieldRow {
            label: TextLine::from(env.clone()),
            help: Some(TextLine::from(format!("Filled from the {account} account when a shell starts."))),
            Label { text: state.to_owned(), role: LabelRole::Secondary }
            TextField {
                label: format!("New value for {env}"),
                kind: FieldKind::Secure,
                text: FieldText::Caller,
                value: draft(),
                oninput: move |text: String| draft.set(text),
                onsubmit: move |_| give.call(()),
            }
            Button {
                label: act.to_owned(),
                title: Some("Keep the typed value as the secret".to_owned()),
                common: named(format!("{act} {env}")),
                onclick: move |_| give.call(()),
            }
            Button {
                label: "Remove".to_owned(),
                title: Some("Forget this secret and stop filling the variable".to_owned()),
                common: named(format!("Remove credential {env}")),
                onclick: move |_| onremove.call(()),
            }
        }
    }
}

/// The row that adds a credential: the variable it fills and its secret.
#[component]
#[allow(non_snake_case)]
pub fn AddCredential(onadd: EventHandler<(String, String)>) -> Element {
    let mut name = use_signal(String::new);
    let mut secret = use_signal(String::new);
    let add = Callback::new(move |(): ()| {
        let (n, s) = (name(), secret());
        secret.set(String::new());
        name.set(String::new());
        onadd.call((n, s));
    });
    rsx! {
        FieldRow {
            label: "New secret",
            help: Some(TextLine::from("A variable such as API_KEY and its secret. The secret is kept apart from this file and not shown again.")),
            TextField {
                label: "New secret name".to_owned(),
                value: name(),
                oninput: move |text: String| name.set(text),
            }
            TextField {
                label: "New secret value".to_owned(),
                kind: FieldKind::Secure,
                text: FieldText::Caller,
                value: secret(),
                oninput: move |text: String| secret.set(text),
                onsubmit: move |_| add.call(()),
            }
            Button {
                label: "Add".to_owned(),
                title: Some("Keep this secret and fill the variable with it".to_owned()),
                common: named("Add secret".to_owned()),
                onclick: move |_| add.call(()),
            }
        }
    }
}
