//! What the window tells the person about their settings at start: plain sentences, a setting
//! named as the Settings window labels it, never a key, a file format or a reader's error.

use std::path::Path;

use crate::edge::Ambient;
use crate::settings::load_settings;

fn ambient(scratch: &Path) -> Ambient {
    let home = scratch.join("home");
    Ambient {
        home: home.clone(),
        env: temor_panes::shell::ChildEnv::new(),
        config_dir: scratch.join("config/temor"),
        runtime_dir: home.join("runtime"),
    }
}

/// The sentences a launch over these files says; `None` leaves a file out.
fn said(settings: Option<&str>, keys: Option<&str>, old: Option<&str>) -> Vec<String> {
    let dir = tempfile::tempdir().expect("scratch");
    let ambient = ambient(dir.path());
    std::fs::create_dir_all(ambient.settings_dir()).expect("folder");
    if let Some(text) = settings {
        std::fs::write(ambient.settings_dir().join("settings.toml"), text).expect("settings");
    }
    if let Some(text) = keys {
        std::fs::write(ambient.keys_path(), text).expect("keys");
    }
    if let Some(text) = old {
        std::fs::write(ambient.old_settings_path(), text).expect("old");
    }
    let said = load_settings(&ambient).expect("loads").told.sentences();
    for sentence in &said {
        for raw in ["`", "font.size", "invalid", "expected", "TOML", "line "] {
            assert!(!sentence.contains(raw), "{raw:?} in {sentence:?}");
        }
    }
    said
}

#[test]
fn settings_in_force_say_nothing() {
    assert!(said(None, None, None).is_empty());
    assert!(said(Some("[font]\nsize = 15\n"), Some("# none\n"), None).is_empty());
    // A key this version does not read is not the person's problem.
    assert!(said(Some("[dock]\npuppy = 1\n"), None, None).is_empty());
}

#[test]
fn a_file_that_cannot_be_read_says_the_defaults_are_in_use() {
    let got = said(Some("[font\nsize = = 3\n"), None, None);
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].contains("default settings"), "{got:?}");
}

#[test]
fn a_refused_value_is_named_as_the_settings_window_names_it() {
    let one = said(Some("[font]\nsize = \"huge\"\n"), None, None);
    assert_eq!(
        one,
        ["“Font size” couldn't be used as you set it, so it's back to its default."]
    );
    let two = said(
        Some("[font]\nsize = \"huge\"\n[cursor]\nstyle = \"star\"\n"),
        None,
        None,
    );
    assert_eq!(two.len(), 1, "{two:?}");
    assert!(
        two[0].starts_with("Some settings couldn't be used"),
        "{two:?}"
    );
    assert!(
        two[0].contains("Font size") && two[0].contains("Cursor shape"),
        "{two:?}"
    );
}

#[test]
fn refused_shortcuts_are_counted_not_quoted() {
    let got = said(
        None,
        Some("temor.nope = Primary+K\ncopy = Primary+Hyper+K\n"),
        None,
    );
    assert_eq!(
        got,
        ["2 of your keyboard shortcuts couldn't be used, so those actions keep their usual keys."]
    );
}

#[test]
fn a_conversion_says_where_the_old_file_went_and_counts_what_stayed_behind() {
    let got = said(None, None, Some("font.size = 15\nnot a setting\n"));
    assert_eq!(
        got[0],
        "temor moved your settings to a new file and kept the old one as terminal.conf.old."
    );
    assert_eq!(got[1..], ["One old setting couldn't be carried over."]);
}
