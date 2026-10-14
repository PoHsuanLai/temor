//! The Spaces page: for each Space, under its name, the folder new tabs open in, the profile new
//! shells start with and the routes that send new sessions to it (PLAN §7). Edits are sent to the
//! main window (`board`), which applies them through the kit's handle and writes `spaces.json`
//! as it does for the Space menu's own edits.
//!
//! A profile secret is never shown: a variable's value is not on the page, only that it is set,
//! and a credential shows the account it is filled from.

use dioxus::prelude::*;
use ds::components::app::spaces::SpaceId;
use ds::components::content::label::LabelRole;
use ds::components::fields::field_row::FieldRow;
use ds::prelude::*;
use temor_core::vocab::text::{KeyPath, ProjectRoot, RelPath};

use super::board::{SpaceBoard, SpaceEdit, SpaceRow};
use super::space_edit::{
    SpaceChange, dotenv_warning, optional, root_warning, route_warning, routes_edited, routes_with,
    routes_without,
};

/// What a row sends: which Space, and the change.
type Sent = (SpaceId, SpaceChange);

/// The page.
#[component]
#[allow(non_snake_case)]
pub fn SpacesPage() -> Element {
    let board = use_hook(|| try_consume_context::<SpaceBoard>());
    let mut rows = use_signal({
        let board = board.clone();
        move || board.as_ref().map(SpaceBoard::rows).unwrap_or_default()
    });
    use_future({
        let board = board.clone();
        move || {
            let board = board.clone();
            async move {
                let Some(board) = board else { return };
                let mut heard = board.heard_rows();
                rows.set(board.rows());
                while heard.changed().await.is_ok() {
                    rows.set(board.rows());
                }
            }
        }
    });
    let Some(board) = board else {
        return rsx! {
            Form {
                FormSection { title: Some("Spaces".to_owned()),
                    FieldRow { label: "Spaces", help: Some(TextLine::from("They can be changed in the main window.")),
                        Label { text: "Not available here".to_owned(), role: LabelRole::Secondary }
                    }
                }
            }
        };
    };
    let send = Callback::new(move |(space, change): Sent| board.send(SpaceEdit { space, change }));
    let sections: Vec<Element> = rows()
        .into_iter()
        .map(|row| {
            let id = row.id.0;
            rsx! {
                SpaceSection { key: "{id}", row, onchange: send }
            }
        })
        .collect();
    rsx! {
        Form { {sections.into_iter()} }
    }
}

/// One Space.
#[component]
#[allow(non_snake_case)]
fn SpaceSection(row: SpaceRow, onchange: Callback<Sent>) -> Element {
    let SpaceRow { id, name, project } = row;
    let change = move |change: SpaceChange| onchange.call((id, change));
    let root = project
        .root
        .as_ref()
        .map(|r| r.as_str().to_owned())
        .unwrap_or_default();
    let key = project
        .profile
        .ssh_identity
        .as_ref()
        .map(|k| k.as_str().to_owned())
        .unwrap_or_default();
    let dotenv = project
        .profile
        .dotenv
        .as_ref()
        .map(|f| f.as_str().to_owned())
        .unwrap_or_default();
    let routes = project.routes.clone();
    rsx! {
        FormSection { title: Some(name),
            SpaceText {
                label: "Folder",
                help: "New tabs open here. Leave it empty for your home folder.",
                placeholder: "Your home folder",
                warning: root_warning(&root),
                value: root,
                onchange: move |text: String| {
                    change(SpaceChange::Root(optional(&text).map(ProjectRoot)));
                },
            }
            SpaceText {
                label: "SSH key",
                help: "The key this Space's own agent holds.",
                placeholder: "No key",
                warning: None,
                value: key,
                onchange: move |text: String| {
                    change(SpaceChange::SshIdentity(optional(&text).map(KeyPath)));
                },
            }
            SpaceText {
                label: "Environment file",
                help: "A file inside the folder whose variables new shells get.",
                placeholder: "No file",
                warning: dotenv_warning(&dotenv),
                value: dotenv,
                onchange: move |text: String| {
                    change(SpaceChange::Dotenv(optional(&text).map(RelPath)));
                },
            }
            for (variable, _) in project.profile.env.iter() {
                FieldRow {
                    key: "env-{variable.as_str()}",
                    label: TextLine::from(variable.as_str().to_owned()),
                    help: Some(TextLine::from("Set in every new shell. Its value is not shown.")),
                    Label { text: "Set".to_owned(), role: LabelRole::Secondary }
                }
            }
            for credential in project.profile.credentials.iter() {
                FieldRow {
                    key: "credential-{credential.env.as_str()}",
                    label: TextLine::from(credential.env.as_str().to_owned()),
                    help: Some(TextLine::from(format!(
                        "Filled from the {} account when a shell starts.",
                        credential.account.as_str()
                    ))),
                    Label { text: "Set".to_owned(), role: LabelRole::Secondary }
                }
            }
            for (at, route) in routes.iter().enumerate() {
                RouteRow {
                    key: "{at}-{route.pattern.as_str()}",
                    pattern: route.pattern.as_str().to_owned(),
                    onedit: {
                        let routes = routes.clone();
                        move |text: String| change(SpaceChange::Routes(routes_edited(routes.clone(), at, &text)))
                    },
                    onremove: {
                        let routes = routes.clone();
                        move |()| change(SpaceChange::Routes(routes_without(routes.clone(), at)))
                    },
                }
            }
            AddRoute {
                onadd: {
                    let routes = routes.clone();
                    move |text: String| change(SpaceChange::Routes(routes_with(routes.clone(), &text)))
                },
            }
        }
    }
}

/// A row of one line of text, kept when the caret leaves or Return is pressed.
#[component]
#[allow(non_snake_case)]
fn SpaceText(
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
fn RouteRow(pattern: String, onedit: EventHandler<String>, onremove: EventHandler<()>) -> Element {
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
fn AddRoute(onadd: EventHandler<String>) -> Element {
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
