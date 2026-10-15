//! The Settings window's edits on files in a scratch folder: a setting saved through the store, a
//! key changed in `keys.conf`. (The windows themselves are in `tests/it/settings.rs`.)

use std::path::Path;

use chordkit::{Chord, Key, Modifier, Modifiers, Overrides, Platform};
use ds_settings::schema::KeyPath;
use ds_settings_page::SettingEdit;
use temor_pane::keys::Action;

use super::edit::{applied, current, save_edit};
use super::keys_edit::{KeyChange, Mine, change_keys, changed_text, load_view};
use super::open::raise;
use crate::edge::{Ambient, keymap_for};
use crate::error::UiError;
use crate::settings::TerminalDoc;

/// An ambient whose config folder is `<scratch>/config/temor`, as on a real machine.
fn ambient(scratch: &Path) -> Ambient {
    let home = scratch.join("home");
    Ambient {
        home: home.clone(),
        env: temor_shell::ChildEnv::new(),
        config_dir: scratch.join("config/temor"),
        runtime_dir: home.join("runtime"),
    }
}

fn edit(path: &str, value: toml::Value) -> SettingEdit {
    SettingEdit {
        path: KeyPath(path.to_owned()),
        value,
    }
}

fn chord(mods: &[Modifier], key: char) -> Chord {
    Chord::new(mods.iter().copied().collect::<Modifiers>(), Key::Char(key))
}

#[test]
fn an_edit_reaches_the_document_or_is_refused_in_words() {
    use toml::Value::{Array, Float, Integer, String as Text};
    let words = |list: &[&str]| Array(list.iter().map(|w| Text((*w).to_owned())).collect());
    // (the edit, what the document then holds as text of that key, or None when refused)
    let rows = [
        (edit("font.size", Float(18.0)), true),
        (edit("font.size", Float(3.0)), false),
        (edit("cursor.style", Text("bar".to_owned())), true),
        (edit("cursor.style", Text("sparkle".to_owned())), false),
        (edit("notify.after", Integer(300)), true),
        (edit("notify.after", Integer(999_999)), false),
        (edit("offer.add", words(&["kubectl debug -it"])), true),
    ];
    for (change, accepted) in rows {
        let got = applied(&TerminalDoc::default(), &change);
        assert_eq!(got.is_ok(), accepted, "{change:?}");
        if let Err(why) = got {
            assert!(matches!(why, UiError::SettingsRefused(_)), "{why:?}");
            assert!(why.to_string().contains(&change.path.0), "{why}");
        }
    }
}

#[test]
fn a_saved_edit_lands_in_the_file_over_what_it_holds_now() {
    let dir = tempfile::tempdir().expect("scratch");
    let ambient = ambient(dir.path());
    let first = save_edit(&ambient, &edit("font.size", toml::Value::Float(20.0))).expect("saved");
    assert_eq!(first.font.size.0.points(), 20.0);
    // Another program changes the cursor; the next edit from this window keeps it.
    let mut other = current(&ambient);
    other.cursor.style = crate::settings::CursorShape::Bar;
    ambient.settings_store().save(&other).expect("saved");
    let next = save_edit(
        &ambient,
        &edit("shell.program", toml::Value::String("fish".into())),
    )
    .expect("saved");
    assert_eq!(next.cursor.style, crate::settings::CursorShape::Bar);
    assert_eq!(current(&ambient), next);
    assert_eq!(next.font.size.0.points(), 20.0);
    // A refused edit leaves the file alone.
    let refused = save_edit(&ambient, &edit("font.size", toml::Value::Float(500.0)));
    assert!(refused.is_err());
    assert_eq!(current(&ambient), next);
}

#[test]
fn a_key_is_bound_reset_and_refused_by_the_keymap_in_force() {
    let (keymap, _) = keymap_for(Platform::Windows, &Overrides::default());
    let ctrl_shift_alt = [Modifier::Ctrl, Modifier::Shift, Modifier::Alt];
    let bound = changed_text(
        "# mine\nbogus = 1\n",
        &keymap,
        KeyChange::Bind(Action::SplitRight, chord(&ctrl_shift_alt, 'n')),
    )
    .expect("free chord");
    assert!(
        bound.contains("temor.split-right = Primary+Alt+N"),
        "{bound}"
    );
    assert!(bound.starts_with("# mine\nbogus = 1\n"), "{bound}");

    // Split right's own default chord is split down's to take only by refusal naming it.
    let held = changed_text(
        "",
        &keymap,
        KeyChange::Bind(
            Action::SplitDown,
            chord(&[Modifier::Ctrl, Modifier::Shift], 'd'),
        ),
    )
    .expect_err("held by split right");
    assert!(held.to_string().contains("Split right"), "{held}");

    let reset = changed_text(&bound, &keymap, KeyChange::Reset(Action::SplitRight)).expect("reset");
    assert_eq!(reset, "# mine\nbogus = 1\n");
    let all = changed_text(&bound, &keymap, KeyChange::ResetAll).expect("reset all");
    assert!(!all.contains("split-right"), "{all}");
}

#[test]
fn keys_conf_is_written_and_read_back_as_the_keymap_the_panes_will_build() {
    let dir = tempfile::tempdir().expect("scratch");
    let ambient = ambient(dir.path());
    let platform = Platform::Windows;
    let before = load_view(&ambient, platform).expect("no file is fine");
    assert_eq!(before.mine(Action::SplitRight), Mine::AsShipped);

    let ctrl_shift_alt = [Modifier::Ctrl, Modifier::Shift, Modifier::Alt];
    let change = KeyChange::Bind(Action::SplitRight, chord(&ctrl_shift_alt, 'n'));
    let after = change_keys(&ambient, platform, change).expect("bound");
    assert_eq!(after.mine(Action::SplitRight), Mine::Changed);
    assert_eq!(
        after.chord_of(Action::SplitRight),
        Some(chord(&ctrl_shift_alt, 'n'))
    );
    let text = std::fs::read_to_string(ambient.keys_path()).expect("keys.conf");
    assert!(text.contains("temor.split-right"), "{text}");

    // A refusal changes nothing on disk.
    let held = KeyChange::Bind(
        Action::SplitDown,
        chord(&[Modifier::Ctrl, Modifier::Shift], 'd'),
    );
    let refused = change_keys(&ambient, platform, held).expect_err("held");
    assert!(refused.to_string().contains("Split right"), "{refused}");
    assert_eq!(
        std::fs::read_to_string(ambient.keys_path()).expect("keys.conf"),
        text
    );

    let cleared = change_keys(&ambient, platform, KeyChange::ResetAll).expect("reset all");
    assert!(cleared.changed.entries().is_empty());
}

#[test]
fn a_window_that_is_opening_or_open_is_raised_and_a_closed_one_is_opened_again() {
    use ds_blitz::WindowLife::{Closed, Open, Opening};
    for (life, raised) in [(Opening, true), (Open, true), (Closed, false)] {
        assert_eq!(raise(life), raised, "{life:?}");
    }
}
