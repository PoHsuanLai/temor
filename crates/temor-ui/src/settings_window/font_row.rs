//! The Font row: a pop-up of the fonts this computer has, monospace ones by default, with "System
//! default" first, a check box for the others, and a line of sample text drawn in the chosen
//! family. A name in the file that is not installed is kept in the list, marked, and the row's
//! help says what the terminal does instead.
//!
//! Which key is this row's is told by its path, the one place that knows (`is_font_key`), until
//! quire's schema can name a control for a key (`control = "..."`, after the pin reaches it).

use dioxus::prelude::*;
use ds::base::vocab::Check;
use ds::components::controls::checkbox::Checkbox;
use ds::components::fields::field_row::FieldRow;
use ds::components::menus::pop_up_button::PopUpButton;
use ds::prelude::*;
use ds_settings::schema::KeySpec;
use ds_settings_page::SettingEdit;
use temor_render::font_catalog;

use crate::view::names::named;

/// The stored word for the computer's own font.
const SYSTEM: &str = "system";

/// Whether `spec` is the terminal's font family.
pub fn is_font_key(spec: &KeySpec) -> bool {
    spec.path.0 == "font.family"
}

/// What the pop-up lists, as (stored word, title): the system's font, then the stored name if
/// it is not among `names` (marked when it is not installed), then `names`.
pub fn font_choices(
    default: Option<&str>,
    names: &[&str],
    stored: &str,
    installed: bool,
) -> Vec<(String, String)> {
    let system = match default {
        Some(name) => format!("System default ({name})"),
        None => "System default".to_owned(),
    };
    let mut out = vec![(SYSTEM.to_owned(), system)];
    let listed = stored == SYSTEM || names.contains(&stored);
    if !listed {
        let title = match installed {
            true => stored.to_owned(),
            false => format!("{stored} (not installed)"),
        };
        out.push((stored.to_owned(), title));
    }
    out.extend(
        names
            .iter()
            .map(|name| ((*name).to_owned(), (*name).to_owned())),
    );
    out
}

/// The help line: the key's own, or what happens to a name that is not installed.
pub fn font_help(own: &str, stored: &str, installed: bool) -> String {
    match installed || stored == SYSTEM {
        true => own.to_owned(),
        false => format!("{stored} is not installed here, so the terminal uses the system's font."),
    }
}

/// The CSS family list for the sample: the chosen family, then the system's, then `monospace`.
/// A name with a quote or a backslash is left out rather than escaped.
pub fn sample_family(chosen: Option<&str>, default: Option<&str>) -> String {
    let quoted = |name: &str| (!name.contains(['\'', '"', '\\'])).then(|| format!("'{name}'"));
    let mut families: Vec<String> = chosen.and_then(quoted).into_iter().collect();
    families.extend(default.and_then(quoted));
    families.push("monospace".to_owned());
    families.join(", ")
}

#[component]
#[allow(non_snake_case)]
pub fn FontRow(spec: KeySpec, value: toml::Value, onedit: EventHandler<SettingEdit>) -> Element {
    let catalog = font_catalog();
    let stored = value.as_str().unwrap_or(SYSTEM).trim().to_owned();
    let installed = stored == SYSTEM || catalog.has(&stored);
    let mut all = use_signal(|| !catalog.is_monospace(&stored) && stored != SYSTEM && installed);
    let names: Vec<&str> = match all() {
        true => catalog.families().collect(),
        false => catalog.monospace().collect(),
    };
    let items: Vec<MenuItem<String>> =
        font_choices(catalog.default_family(), &names, &stored, installed)
            .into_iter()
            .map(|(word, title)| MenuItem::new(word, title))
            .collect();
    let label = TextLine::from(spec.label.0.clone());
    let help = TextLine::from(font_help(&spec.help.0, &stored, installed));
    let path = spec.path.clone();
    let chosen = (stored != SYSTEM && installed).then_some(stored.as_str());
    let family = sample_family(chosen, catalog.default_family());
    rsx! {
        FieldRow { label, help: Some(help),
            PopUpButton {
                common: named("Font"),
                items,
                value: Some(stored.clone()),
                onpick: move |word: String| {
                    onedit.call(SettingEdit {
                        path: path.clone(),
                        value: toml::Value::String(word),
                    });
                },
            }
            Checkbox {
                label: "Show all fonts",
                value: if all() { Check::On } else { Check::Off },
                onchange: move |next: Check| all.set(next == Check::On),
            }
        }
        FieldRow { label: TextLine::from("Sample"),
            span {
                class: "temor-font-sample",
                style: "font-family: {family}",
                "The quick brown fox 0O1lI {{ }} [ ] ~/ -> =>"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_leads_with_the_system_font_and_keeps_a_stored_name_it_does_not_have() {
        let names = ["Menlo", "SF Mono"];
        // (what, default, stored, installed, the first titles)
        type Case<'a> = (&'a str, Option<&'a str>, &'a str, bool, Vec<&'a str>);
        let cases: [Case; 4] = [
            (
                "system",
                Some("Menlo"),
                "system",
                true,
                vec!["System default (Menlo)", "Menlo"],
            ),
            (
                "listed",
                None,
                "Menlo",
                true,
                vec!["System default", "Menlo"],
            ),
            (
                "missing",
                None,
                "Fira Code",
                false,
                vec!["System default", "Fira Code (not installed)", "Menlo"],
            ),
            (
                "installed but not monospace",
                None,
                "Helvetica",
                true,
                vec!["System default", "Helvetica", "Menlo"],
            ),
        ];
        for (what, default, stored, installed, want) in cases {
            let got: Vec<String> = font_choices(default, &names, stored, installed)
                .into_iter()
                .map(|(_, title)| title)
                .take(want.len())
                .collect();
            assert_eq!(got, want, "{what}");
        }
    }

    #[test]
    fn the_help_says_what_happens_to_a_missing_font_and_the_sample_never_breaks_its_quotes() {
        assert_eq!(font_help("own", "Menlo", true), "own");
        assert_eq!(font_help("own", "system", false), "own");
        assert!(font_help("own", "Nope", false).contains("Nope is not installed"));
        assert_eq!(
            sample_family(Some("Menlo"), Some("SF Mono")),
            "'Menlo', 'SF Mono', monospace"
        );
        assert_eq!(sample_family(Some("It's"), None), "monospace");
        assert_eq!(sample_family(None, None), "monospace");
    }
}
