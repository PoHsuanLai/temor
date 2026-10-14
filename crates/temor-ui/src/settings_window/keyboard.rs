//! The Keyboard page: one row for each action that has a key, its shortcut in quire's recorder,
//! Reset on a row the person changed, and Reset All. A click makes the recorder listen and the
//! next combination is offered to the action; a refusal says why under the row, naming the action
//! that already has the chord. Every change is kept at once in `keys.conf` (`keys_edit`); the
//! panes and the command bar follow the file.

use chordkit::Chord;
use dioxus::prelude::*;
use ds::components::fields::field_row::FieldRow;
use ds::prelude::*;
use temor_pane::keys::{Action, bindable};

use super::keys_edit::{KeyChange, Mine, change_keys, load_view};
use crate::view::launch::Launch;

/// What the page says, and under which row (Reset All's when `about` is none).
#[derive(Debug, Clone, PartialEq)]
struct Said {
    about: Option<Action>,
    text: String,
}

/// The name a row carries: the action's, with a tab's number.
pub fn row_name(action: Action) -> String {
    match action {
        Action::SelectTab(n) => format!("{} {n}", action.label()),
        other => other.label().to_owned(),
    }
}

/// The page.
#[component]
pub fn KeyboardPage() -> Element {
    let launch = use_hook(consume_context::<Launch>);
    let platform = launch.keymap.platform();
    let ambient = launch.ambient.clone();
    let mut view = use_signal({
        let ambient = ambient.clone();
        move || load_view(&ambient, platform).ok()
    });
    let mut said = use_signal(|| None::<Said>);
    let change = Callback::new(move |(about, change): (Option<Action>, KeyChange)| {
        match change_keys(&ambient, platform, change) {
            Ok(next) => {
                view.set(Some(next));
                said.set(None);
            }
            Err(why) => said.set(Some(Said {
                about,
                text: why.to_string(),
            })),
        }
    });
    let Some(current) = view() else {
        return rsx! {
            Form {
                FormSection { title: Some("Shortcuts".to_owned()),
                    FieldRow { label: "Shortcuts", help: Some(TextLine::from("The shortcuts file cannot be read.")),
                        Label { text: "Nothing to show".to_owned() }
                    }
                }
            }
        };
    };
    let any_changed = !current.changed.entries().is_empty();
    let about = |action: Option<Action>| {
        said()
            .filter(|said| said.about == action)
            .map(|said| TextLine::from(said.text))
    };
    rsx! {
        Form {
            FormSection {
                title: Some("Shortcuts".to_owned()),
                footer: Some("Click a shortcut, then press the keys you want. Backspace puts the original back.".to_owned()),
                for action in bindable() {
                    KeyRow {
                        key: "{action:?}",
                        action,
                        value: current.chord_of(action),
                        mine: current.mine(action),
                        said: about(Some(action)),
                        onchange: change,
                    }
                }
            }
            FormSection {
                FieldRow {
                    label: "Restore the original shortcuts",
                    help: about(None),
                    Button {
                        label: "Reset All".to_owned(),
                        title: Some("Put every shortcut back as it came".to_owned()),
                        availability: if any_changed { Availability::Enabled } else { Availability::Disabled },
                        onclick: move |_| change.call((None, KeyChange::ResetAll)),
                    }
                }
            }
        }
    }
}

/// One action: its name, its shortcut in a recorder, and Reset when the person changed it.
#[component]
fn KeyRow(
    action: Action,
    value: Option<Chord>,
    mine: Mine,
    said: Option<TextLine>,
    onchange: Callback<(Option<Action>, KeyChange)>,
) -> Element {
    let name = row_name(action);
    rsx! {
        FieldRow { label: TextLine::from(name.clone()), help: said,
            ShortcutField {
                label: format!("Change the shortcut for {name}"),
                value,
                onrecord: move |recorded: ShortcutRecorded| {
                    let asked = match recorded {
                        ShortcutRecorded::Chord(chord) => Some(KeyChange::Bind(action, chord)),
                        ShortcutRecorded::Cleared => Some(KeyChange::Reset(action)),
                        _ => None,
                    };
                    if let Some(asked) = asked {
                        onchange.call((Some(action), asked));
                    }
                },
            }
            if mine == Mine::Changed {
                Button {
                    label: "Reset".to_owned(),
                    title: Some(format!("Put {name} back as it came")),
                    onclick: move |_| onchange.call((Some(action), KeyChange::Reset(action))),
                }
            }
        }
    }
}
