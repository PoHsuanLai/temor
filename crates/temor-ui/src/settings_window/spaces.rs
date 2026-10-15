//! The Spaces page: for each Space, under its name, the folder new tabs open in, the profile new
//! shells start with and the routes that send new sessions to it (PLAN §7). Edits are sent to the
//! main window (`board`), which applies them through the kit's handle and writes `spaces.json`
//! as it does for the Space menu's own edits.
//!
//! Variables are plain name and value fields. A credential's secret is never shown: the page says
//! only whether it is set, and offers Replace and Remove (`space_profile`, `space_secrets`).

use dioxus::prelude::*;
use ds::components::app::spaces::SpaceId;
use ds::components::content::label::LabelRole;
use ds::components::fields::field_row::FieldRow;
use ds::prelude::*;
use temor_core::vocab::text::{KeyPath, ProjectRoot, RelPath};

use super::board::{SpaceBoard, SpaceEdit, SpaceRow};
use super::space_edit::{
    SpaceChange, dotenv_warning, env_renamed, env_valued, env_with, env_without, optional,
    root_warning, routes_edited, routes_with, routes_without,
};
use super::space_fields::{AddRoute, RouteRow, SpaceText};
use super::space_profile::{AddCredential, AddVariable, CredentialRow, VariableRow};
use super::space_secrets;
use crate::app::Desktop;
use crate::edge::{Credentials, VaultCredentials};

/// What a row sends: which Space, and the change.
type Sent = (SpaceId, SpaceChange);

/// The page.
#[component]
#[allow(non_snake_case)]
pub fn SpacesPage() -> Element {
    let board = use_hook(try_consume_context::<SpaceBoard>);
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
    let mut said = use_signal(|| None::<String>);
    let say = Callback::new(move |text: String| said.set(Some(text)));
    let send = Callback::new(move |(space, change): Sent| {
        said.set(None);
        board.send(SpaceEdit { space, change });
    });
    let sections: Vec<Element> = rows()
        .into_iter()
        .map(|row| {
            let id = row.id.0;
            rsx! {
                SpaceSection { key: "{id}", row, onchange: send, onsay: say }
            }
        })
        .collect();
    rsx! {
        if let Some(text) = said() {
            InlineBanner {
                severity: Severity::Danger,
                text,
                onclose: move |_| said.set(None),
            }
        }
        Form { {sections.into_iter()} }
    }
}

/// One Space.
#[component]
#[allow(non_snake_case)]
fn SpaceSection(row: SpaceRow, onchange: Callback<Sent>, onsay: Callback<String>) -> Element {
    let SpaceRow { id, name, project } = row;
    let change = move |change: SpaceChange| onchange.call((id, change));
    let store = use_hook(|| match try_consume_context::<Credentials>() {
        Some(given) => given,
        None => try_consume_context::<Desktop>()
            .map(|desktop| VaultCredentials::credentials(desktop.secrets.clone()))
            .unwrap_or_default(),
    });
    // Variables and credentials: each handler hands the new whole list on.
    let env = project.profile.env.clone();
    let credentials = project.profile.credentials.clone();
    let try_env = move |next: Result<Vec<_>, String>| match next {
        Ok(env) => change(SpaceChange::Env(env)),
        Err(why) => onsay.call(why),
    };
    let try_credentials = move |next: Result<Vec<_>, String>| match next {
        Ok(list) => change(SpaceChange::Credentials(list)),
        Err(why) => onsay.call(why),
    };
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
            for (at, (variable, value)) in env.iter().enumerate() {
                VariableRow {
                    key: "env-{at}-{variable.as_str()}={value.0}",
                    name: variable.as_str().to_owned(),
                    value: value.0.clone(),
                    onname: {
                        let env = env.clone();
                        move |text: String| try_env(env_renamed(env.clone(), at, &text))
                    },
                    onvalue: {
                        let env = env.clone();
                        move |text: String| try_env(Ok(env_valued(env.clone(), at, &text)))
                    },
                    onremove: {
                        let env = env.clone();
                        move |()| try_env(Ok(env_without(env.clone(), at)))
                    },
                }
            }
            AddVariable {
                onadd: {
                    let env = env.clone();
                    move |(name, value): (String, String)| {
                        try_env(env_with(env.clone(), &name, &value));
                    }
                },
            }
            for credential in credentials.iter() {
                CredentialRow {
                    key: "credential-{credential.env.as_str()}",
                    env: credential.env.as_str().to_owned(),
                    account: credential.account.as_str().to_owned(),
                    held: space_secrets::held(&*store.0, &credential.account),
                    onset: {
                        let (store, account) = (store.clone(), credential.account.clone());
                        move |secret: String| {
                            if let Err(why) = space_secrets::replace(&*store.0, &account, &secret) {
                                onsay.call(why);
                            }
                        }
                    },
                    onremove: {
                        let (store, list, env) =
                            (store.clone(), credentials.clone(), credential.env.clone());
                        move |()| try_credentials(space_secrets::remove(&*store.0, list.clone(), &env))
                    },
                }
            }
            AddCredential {
                onadd: {
                    let (store, list) = (store.clone(), credentials.clone());
                    move |(name, secret): (String, String)| {
                        try_credentials(space_secrets::add(&*store.0, id, list.clone(), &name, &secret));
                    }
                },
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
