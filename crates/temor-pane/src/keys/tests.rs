//! Unit tests for the pane's pure parts.

use super::action::Action;
use super::chord::{Chord, ChordKey, Mods, NamedKey};
use super::keymap::Keymap;
use crate::error::SettingsError;
use crate::settings::{TerminalSettings, parse_settings};

fn ctrl_shift(c: char) -> Chord {
    Chord::of_char(c, Mods::CTRL.with(Mods::SHIFT))
}

#[test]
fn chords_parse_in_any_case_and_order_of_modifiers() {
    let parsed = Chord::parse("Shift+Ctrl+C").expect("valid chord");
    assert_eq!(parsed, ctrl_shift('c'));
    assert_eq!(
        Chord::parse("alt+enter").map(|c| c.key),
        Ok(ChordKey::Named(NamedKey::Enter))
    );
    assert!(matches!(Chord::parse(""), Err(SettingsError::BadChord(_))));
    assert!(matches!(
        Chord::parse("hyper+c"),
        Err(SettingsError::BadChord(_))
    ));
    assert!(matches!(
        Chord::parse("ctrl+"),
        Err(SettingsError::BadChord(_))
    ));
}

#[test]
fn chord_labels_show_the_command_key_for_ctrl_shift() {
    assert_eq!(ctrl_shift('c').label(), "⌘C");
    assert_eq!(
        Chord::parse("ctrl+alt+g").map(|c| c.label()),
        Ok("⌃⌥G".to_owned())
    );
    assert_eq!(
        Chord::parse("alt+enter").map(|c| c.label()),
        Ok("⌥↩".to_owned())
    );
}

#[test]
fn super_and_ctrl_space_are_reserved() {
    assert!(Chord::parse("super+c").is_ok_and(|c| c.is_reserved()));
    assert!(Chord::parse("ctrl+space").is_ok_and(|c| c.is_reserved()));
    assert!(!ctrl_shift('c').is_reserved());
}

#[test]
fn defaults_bind_each_action_once_and_never_use_super() {
    let keymap = Keymap::defaults();
    for action in Action::ALL {
        let chord = keymap
            .chord_for(*action)
            .expect("every action has a default");
        assert_eq!(keymap.action_for(chord), Some(*action));
        assert!(!chord.is_reserved());
    }
    assert_eq!(keymap.action_for(&ctrl_shift('c')), Some(Action::Copy));
}

#[test]
fn rebinding_moves_an_action_and_refuses_clashes() {
    let mut keymap = Keymap::defaults();
    let alt_c = Chord::parse("ctrl+alt+c").expect("valid");
    keymap.bind(Action::Copy, alt_c).expect("free chord");
    assert_eq!(keymap.action_for(&alt_c), Some(Action::Copy));
    assert_eq!(keymap.action_for(&ctrl_shift('c')), None);
    let clash = keymap.bind(Action::Paste, alt_c);
    assert!(matches!(clash, Err(SettingsError::ChordInUse { .. })));
    let reserved = keymap.bind(Action::Paste, Chord::parse("super+v").expect("valid"));
    assert!(matches!(reserved, Err(SettingsError::ReservedChord(_))));
}

#[test]
fn settings_text_applies_good_lines_and_reports_bad_ones() {
    let text = "# comment\nfont.size = 15\nscrollback.lines = 500\ncursor.style = bar\n\
                font.family = \"Iosevka\"\nshell.program = fish\nkeys.copy = ctrl+alt+c\n\
                font.size = huge\nkeys.paste = super+v\nbogus = 1\n";
    let parsed = parse_settings(text);
    assert_eq!(parsed.settings.font_size.points(), 15.0);
    assert_eq!(parsed.settings.scrollback.lines(), 500);
    assert_eq!(parsed.settings.keymap.action_for(&ctrl_shift('c')), None);
    let lines: Vec<usize> = parsed.issues.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![8, 9, 10]);
}

#[test]
fn settings_values_have_limits() {
    assert!(parse_settings("font.size = 5").issues.len() == 1);
    assert!(parse_settings("font.size = 72").issues.is_empty());
    assert!(parse_settings("scrollback.lines = 1000001").issues.len() == 1);
    assert!(parse_settings("cursor.style = blink").issues.len() == 1);
    assert_eq!(parse_settings("").settings.scrollback.lines(), 10_000);
}

#[test]
fn prompt_jumps_are_bound_to_ctrl_shift_arrows_and_settable() {
    let keymap = Keymap::defaults();
    let up = Chord::parse("ctrl+shift+up").expect("valid");
    let down = Chord::parse("ctrl+shift+down").expect("valid");
    assert_eq!(keymap.action_for(&up), Some(Action::PreviousPrompt));
    assert_eq!(keymap.action_for(&down), Some(Action::NextPrompt));
    assert_eq!(up.label(), "⌘↑");
    let parsed = parse_settings("keys.next_prompt = alt+down");
    assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
    assert_eq!(
        parsed.settings.keymap.chord_for(Action::NextPrompt),
        Some(&Chord::parse("alt+down").expect("valid"))
    );
}

#[test]
fn notify_after_reads_seconds_and_off() {
    use crate::settings::NotifyAfter;
    assert_eq!(
        TerminalSettings::default().notify_after,
        NotifyAfter::Seconds(10)
    );
    let parsed = parse_settings("notify.after = 30\n");
    assert_eq!(parsed.settings.notify_after, NotifyAfter::Seconds(30));
    assert_eq!(
        parse_settings("notify.after = off").settings.notify_after,
        NotifyAfter::Off
    );
    assert_eq!(
        parse_settings("notify.after = 0").settings.notify_after,
        NotifyAfter::Off
    );
    assert_eq!(parse_settings("notify.after = soon").issues.len(), 1);
}
