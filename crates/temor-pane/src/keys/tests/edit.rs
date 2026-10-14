//! Writing the key file: a binding, a reset, and the refusals that name what is in the way.

use chordkit::Overrides;

use super::super::{Action, bind_key, reset_all_keys, reset_key};
use super::support::{OURS, keymap_for};
use crate::error::SettingsError;

const FILE: &str = "# my keys\ntemor.split-right = Primary+Alt+N\n\nunknown.thing = Primary+K\ncopy = Primary+Hyper+K # old\n";

#[test]
fn a_binding_replaces_the_actions_older_lines_and_keeps_the_rest() {
    let keymap = keymap_for(OURS);
    let text = bind_key(FILE, &keymap, Action::SplitRight, "Primary+Alt+M").expect("free");
    assert!(text.starts_with("# my keys\n"), "{text}");
    assert!(text.contains("unknown.thing = Primary+K\n"), "{text}");
    assert!(!text.contains("Primary+Alt+N"), "{text}");
    assert!(
        text.ends_with("temor.split-right = Primary+Alt+M\n"),
        "{text}"
    );
    // It reads back as one chord on the action.
    let (overrides, _) = Overrides::parse(&text);
    let split = overrides
        .entries()
        .iter()
        .find(|entry| entry.action.id() == "temor.split-right")
        .expect("the binding");
    assert_eq!(split.chords.len(), 1);
}

#[test]
fn a_refused_binding_names_why_and_leaves_the_file_alone() {
    let keymap = keymap_for(OURS);
    // (action, chord, what the refusal is)
    for (action, chord, held_by) in [
        // Primary+D is where "Split right" is.
        (Action::OpenBar, "Primary+D", Some("Split right")),
        (Action::OpenBar, "Primary+Nope", None),
        (Action::CloseFind, "Primary+K", None),
    ] {
        let err = bind_key(FILE, &keymap, action, chord).expect_err(chord);
        match (held_by, &err) {
            (Some(name), SettingsError::KeyHeld { held_by, .. }) => assert_eq!(held_by, name),
            (None, SettingsError::Key(_) | SettingsError::KeyNotBindable(_)) => {}
            other => panic!("{action:?} {chord}: {other:?}"),
        }
    }
}

#[test]
fn a_reset_drops_one_actions_lines_or_every_key_line() {
    assert_eq!(
        reset_key(FILE, Action::SplitRight),
        "# my keys\n\nunknown.thing = Primary+K\ncopy = Primary+Hyper+K # old\n"
    );
    assert_eq!(reset_key(FILE, Action::Paste), FILE);
    assert_eq!(
        reset_all_keys(FILE),
        "# my keys\n\nunknown.thing = Primary+K\n"
    );
}
