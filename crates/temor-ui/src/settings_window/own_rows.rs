//! The rows of keys quire's `SettingsPage` only shows read-only: free text, a bounded number and
//! a list of words (a pop-up button or a switch it draws itself). Each is chosen by the key's
//! kind in the schema, never by its name, and hands a [`SettingEdit`] back as the page does.
//!
//! Pure rows; saving is the page's (`general`).

use dioxus::prelude::*;
use ds::components::content::label::LabelRole;
use ds::components::fields::field_row::FieldRow;
use ds::prelude::*;
use ds_settings::schema::{KeyKind, KeySpec};
use ds_settings_page::SettingEdit;

use super::row_parts::{AddButton, RemoveButton};

/// What a number field takes, in words: `a number from 6 to 72 points`.
pub fn number_wanted(min: i64, max: i64, unit: Option<&str>) -> String {
    match unit {
        Some(unit) => format!("a number from {min} to {max} {unit}"),
        None => format!("a number from {min} to {max}"),
    }
}

/// The value a number field's `text` means for a key whose stored value is `current`: a whole
/// number, or a decimal where the file holds one. `None` when it is not a number in `min..=max`.
pub fn number_value(text: &str, current: &toml::Value, min: i64, max: i64) -> Option<toml::Value> {
    let text = text.trim();
    let parsed = text.parse::<f64>().ok().filter(|n| n.is_finite())?;
    if parsed < min as f64 || parsed > max as f64 {
        return None;
    }
    match current {
        toml::Value::Float(_) => Some(toml::Value::Float(parsed)),
        _ if parsed.fract() == 0.0 => Some(toml::Value::Integer(parsed as i64)),
        _ => None,
    }
}

/// The text a number is shown as: a whole decimal without its `.0`.
fn number_text(value: &toml::Value) -> String {
    match value {
        toml::Value::Float(n) if n.fract() == 0.0 => format!("{}", *n as i64),
        toml::Value::Float(n) => n.to_string(),
        toml::Value::Integer(n) => n.to_string(),
        other => other.as_str().unwrap_or_default().to_owned(),
    }
}

/// The entries of a list value, in order; anything that is not a word is left out.
pub fn entries_of(value: &toml::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn words(entries: Vec<String>) -> toml::Value {
    toml::Value::Array(entries.into_iter().map(toml::Value::String).collect())
}

/// `entries` with `added` at the end, unless it is blank or already there.
pub fn with_entry(mut entries: Vec<String>, added: &str) -> Vec<String> {
    let added = added.split_whitespace().collect::<Vec<_>>().join(" ");
    if !added.is_empty() && !entries.contains(&added) {
        entries.push(added);
    }
    entries
}

/// `entries` without the one at `at`.
pub fn without_entry(mut entries: Vec<String>, at: usize) -> Vec<String> {
    if at < entries.len() {
        entries.remove(at);
    }
    entries
}

/// A row for `spec`, by its kind. `value` is what the file holds; `onsay` hears why a value was
/// not taken before it reached the file.
#[component]
#[allow(non_snake_case)]
pub fn OwnRow(
    spec: KeySpec,
    value: toml::Value,
    onedit: EventHandler<SettingEdit>,
    onsay: EventHandler<String>,
) -> Element {
    match spec.kind.clone() {
        KeyKind::Bounded { min, max, unit } => rsx! {
            NumberRow { spec, value, min, max, unit, onedit, onsay }
        },
        KeyKind::List(_) => rsx! {
            ListRows { spec, value, onedit }
        },
        _ => rsx! {
            TextRow { spec, value, onedit }
        },
    }
}

/// The label and help of a key's row.
fn heading(spec: &KeySpec) -> (TextLine, Option<TextLine>) {
    let help = (!spec.help.0.is_empty()).then(|| TextLine::from(spec.help.0.clone()));
    (TextLine::from(spec.label.0.clone()), help)
}

#[component]
#[allow(non_snake_case)]
fn TextRow(spec: KeySpec, value: toml::Value, onedit: EventHandler<SettingEdit>) -> Element {
    let (label, help) = heading(&spec);
    let stored = value.as_str().unwrap_or_default().to_owned();
    let mut typed = use_signal(|| stored.clone());
    let path = spec.path.clone();
    rsx! {
        FieldRow { label, help,
            TextField {
                label: spec.label.0.clone(),
                value: typed(),
                oninput: move |text: String| typed.set(text),
                onchange: move |text: String| {
                    if text != stored {
                        onedit.call(SettingEdit {
                            path: path.clone(),
                            value: toml::Value::String(text),
                        });
                    }
                },
            }
        }
    }
}

#[component]
#[allow(non_snake_case)]
fn NumberRow(
    spec: KeySpec,
    value: toml::Value,
    min: i64,
    max: i64,
    unit: Option<String>,
    onedit: EventHandler<SettingEdit>,
    onsay: EventHandler<String>,
) -> Element {
    let (label, help) = heading(&spec);
    let shown = number_text(&value);
    let mut typed = use_signal(|| shown.clone());
    let path = spec.path.clone();
    let name = spec.label.0.clone();
    let suffix = unit.clone();
    rsx! {
        FieldRow { label, help,
            TextField {
                label: spec.label.0.clone(),
                value: typed(),
                oninput: move |text: String| typed.set(text),
                suffix: suffix.map(|unit| rsx! { Label { text: unit, role: LabelRole::Secondary } }),
                onchange: move |text: String| {
                    if text.trim() == shown {
                        return;
                    }
                    match number_value(&text, &value, min, max) {
                        Some(next) => onedit.call(SettingEdit { path: path.clone(), value: next }),
                        None => onsay.call(format!(
                            "{name} takes {}.",
                            number_wanted(min, max, unit.as_deref())
                        )),
                    }
                },
            }
        }
    }
}

#[component]
#[allow(non_snake_case)]
fn ListRows(spec: KeySpec, value: toml::Value, onedit: EventHandler<SettingEdit>) -> Element {
    let (label, help) = heading(&spec);
    let entries = entries_of(&value);
    let head = label.clone();
    let mut draft = use_signal(String::new);
    let path = spec.path.clone();
    let add = Callback::new({
        let (entries, path) = (entries.clone(), path.clone());
        move |text: String| {
            let next = with_entry(entries.clone(), &text);
            draft.set(String::new());
            if next.len() != entries.len() {
                onedit.call(SettingEdit {
                    path: path.clone(),
                    value: words(next),
                });
            }
        }
    });
    let remove = Callback::new({
        let (entries, path) = (entries.clone(), path.clone());
        move |at: usize| {
            onedit.call(SettingEdit {
                path: path.clone(),
                value: words(without_entry(entries.clone(), at)),
            });
        }
    });
    rsx! {
        for (at, entry) in entries.iter().enumerate() {
            FieldRow {
                key: "{at}-{entry}",
                label: TextLine::from(entry.clone()),
                help: (at == 0).then(|| head.clone()),
                RemoveButton {
                    what: entry.clone(),
                    tip: "Take this off the list",
                    onclick: move |()| remove.call(at),
                }
            }
        }
        FieldRow { label, help,
            TextField {
                label: spec.label.0.clone(),
                value: draft(),
                placeholder: "A command".to_owned(),
                oninput: move |text: String| draft.set(text),
                onsubmit: move |text: String| add.call(text),
            }
            AddButton {
                what: "command",
                tip: "Add this command to the list",
                onclick: move |()| add.call(draft()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_taken_only_inside_its_range_and_in_the_form_the_file_holds() {
        use toml::Value::{Float, Integer};
        // (text, the file's value, the range, what is taken)
        let rows = [
            ("14", Float(12.0), (6, 72), Some(Float(14.0))),
            (" 9.5 ", Float(12.0), (6, 72), Some(Float(9.5))),
            ("5", Float(12.0), (6, 72), None),
            ("73", Float(12.0), (6, 72), None),
            ("many", Float(12.0), (6, 72), None),
            ("inf", Float(12.0), (6, 72), None),
            ("300", Integer(30), (0, 86_400), Some(Integer(300))),
            ("1.5", Integer(30), (0, 86_400), None),
        ];
        for (text, current, (min, max), want) in rows {
            assert_eq!(number_value(text, &current, min, max), want, "{text:?}");
        }
    }

    #[test]
    fn a_list_takes_a_command_once_and_gives_one_up_by_position() {
        let list = vec!["ssh".to_owned(), "su".to_owned()];
        assert_eq!(
            with_entry(list.clone(), "  kubectl   debug "),
            ["ssh", "su", "kubectl debug"]
        );
        assert_eq!(with_entry(list.clone(), "ssh"), list);
        assert_eq!(with_entry(list.clone(), "   "), list);
        assert_eq!(without_entry(list.clone(), 0), ["su"]);
        assert_eq!(without_entry(list.clone(), 5), list);
    }
}
