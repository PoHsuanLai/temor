//! The rows of a Space's profile that hold variables and credentials. A variable is a name and a
//! value in plain fields. A credential shows only that it is set: its field is a secure one that
//! keeps the typed text out of the markup, it is emptied the moment the value is handed on, and
//! the stored value is never read back into the page.

use dioxus::prelude::*;
use ds::components::content::label::LabelRole;
use ds::components::fields::field_row::FieldRow;
use ds::components::fields::text_field_model::{FieldKind, FieldText};
use ds::prelude::*;

use temor_client::model::space::Credential;

use super::row_parts::{AddButton, KeptField, RemoveButton};
use super::space_secrets;
use crate::edge::Credentials;
use crate::view::names::named;

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
    rsx! {
        FieldRow {
            label: "Variable",
            help: Some(TextLine::from("Set in every new shell.")),
            KeptField { label: format!("Name of {name}"), value: name.clone(), onchange: onname }
            KeptField { label: format!("Value of {name}"), value, onchange: onvalue }
            RemoveButton {
                what: name,
                tip: "Take this variable away",
                onclick: onremove,
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
            AddButton {
                what: "variable",
                tip: "Add this variable",
                onclick: move |()| add.call(()),
            }
        }
    }
}

/// One credential: whether its secret is set, a secure field for a new value, Replace (Set where
/// none is) and Remove. The stored value is not shown. The store is asked on a thread of its own
/// (`Credentials::ask`): the window never waits on the keychain.
#[component]
#[allow(non_snake_case)]
pub fn CredentialRow(
    credential: Credential,
    onsay: Callback<String>,
    onremove: EventHandler<()>,
) -> Element {
    let store = use_context::<Credentials>();
    let mut draft = use_signal(String::new);
    // One more each time the secret is set, so whether it is held is asked again.
    let mut set = use_signal(|| 0u32);
    let held = use_resource({
        let (store, account) = (store.clone(), credential.account.clone());
        move || {
            let _ = set();
            let (store, account) = (store.clone(), account.clone());
            async move {
                store
                    .ask(move |store| space_secrets::held(store, &account))
                    .await
            }
        }
    });
    let held = held.read().flatten();
    let (state, act) = match held {
        Some(true) => ("Set", "Replace"),
        Some(false) => ("Not set", "Set"),
        None => ("\u{2026}", "Set"),
    };
    let env = credential.env.as_str().to_owned();
    let account = credential.account.clone();
    let give = Callback::new(move |(): ()| {
        let secret = draft();
        draft.set(String::new());
        let (store, account) = (store.clone(), account.clone());
        spawn(async move {
            let done = store
                .ask(move |store| space_secrets::replace(store, &account, &secret))
                .await;
            match done {
                Some(Ok(())) => set += 1,
                Some(Err(why)) => onsay.call(why),
                None => onsay.call(NO_THREAD.to_owned()),
            }
        });
    });
    rsx! {
        FieldRow {
            label: TextLine::from(env.clone()),
            help: Some(TextLine::from(format!("Filled from the {} account when a shell starts.", credential.account.as_str()))),
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
            RemoveButton {
                what: format!("credential {env}"),
                tip: "Forget this secret and stop filling the variable",
                onclick: onremove,
            }
        }
    }
}

/// What the person is told when the secret store could not be asked at all.
pub const NO_THREAD: &str = "The password store can't be reached right now.";

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
            AddButton {
                what: "secret",
                tip: "Keep this secret and fill the variable with it",
                onclick: move |()| add.call(()),
            }
        }
    }
}
