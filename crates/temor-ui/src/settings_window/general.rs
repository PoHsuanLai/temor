//! The General and Appearance pages: quire's `SettingsPage` over temor's schema, split by
//! `pages::page_of`. The page draws the switches and choices; the window's own rows draw the
//! text, numbers and lists it only shows (`own_rows`). Every change goes through [`save_edit`]:
//! the file is read again, the edit applied, the result saved. A save that fails shows a banner
//! and changes nothing; one that works is followed live by the panes (`hub::settings_live`), so
//! nothing here applies a setting.

use std::path::PathBuf;

use dioxus::prelude::*;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use ds_settings::schema::KeySpec;
use ds_settings_page::{SectionRows, SettingEdit, SettingsPage};
use temor_panes::pane::keys::SettingsPane;

use super::edit::{save_edit, values_of};
use super::own_rows::OwnRow;
use super::pages::{own_sections, page_schema};
use crate::platform::default_file_shell;
use crate::settings::{TerminalDoc, kept_aside};
use crate::view::launch::Launch;

/// The value at `key`'s path in `values`, else the key's default.
fn value_at(values: &toml::Value, key: &KeySpec) -> toml::Value {
    key.path
        .0
        .split('.')
        .try_fold(values, |at, part| at.get(part))
        .cloned()
        .unwrap_or_else(|| key.default.clone())
}

/// One page of the schema: `pane` is General or Appearance. The document is the window's
/// (`root`), shown from the file and replaced by what a save leaves there.
#[component]
#[allow(non_snake_case)]
pub fn SchemaPage(pane: SettingsPane) -> Element {
    let launch = use_hook(consume_context::<Launch>);
    let mut doc = use_context::<Signal<TerminalDoc>>();
    let mut said = use_signal(|| None::<String>);
    let mut kept = use_signal(|| None::<PathBuf>);
    let values = values_of(&doc.read());
    let save = Callback::new({
        let ambient = launch.ambient.clone();
        move |edit: SettingEdit| match save_edit(&ambient, &edit) {
            Ok(edited) => {
                doc.set(edited.doc);
                said.set(None);
                if edited.kept.is_some() {
                    kept.set(edited.kept);
                }
            }
            Err(why) => said.set(Some(why.to_string())),
        }
    });
    let say = Callback::new(move |text: String| said.set(Some(text)));
    let extras: Vec<SectionRows> = own_sections(pane)
        .into_iter()
        .map(|(title, keys)| {
            let rows: Vec<Element> = keys
                .into_iter()
                .map(|key| {
                    let value = value_at(&values, &key);
                    // Keyed by what it holds, so a value changed by the file starts the row over.
                    let id = format!("{}={value}", key.path.0);
                    rsx! {
                        OwnRow { key: "{id}", spec: key, value, onedit: save, onsay: say }
                    }
                })
                .collect();
            SectionRows::new(title, rsx! { {rows.into_iter()} })
        })
        .collect();
    rsx! {
        if let Some(path) = kept() {
            InlineBanner {
                severity: Severity::Warn,
                text: kept_aside(&path),
                actions: rsx! {
                    Button {
                        label: "Show in Folder".to_owned(),
                        size: ControlSize::Small,
                        onclick: move |_| {
                            let _ = default_file_shell().reveal(&path);
                        },
                    }
                },
                onclose: move |_| kept.set(None),
            }
        }
        if let Some(text) = said() {
            InlineBanner {
                severity: Severity::Danger,
                text,
                onclose: move |_| said.set(None),
            }
        }
        SettingsPage {
            schemas: vec![page_schema(pane)],
            values,
            onedit: save,
            extras,
        }
    }
}
