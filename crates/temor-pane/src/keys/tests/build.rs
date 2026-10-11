//! The action ids a person's file uses, how key lines read, and how shortcuts are shown.

use chordkit::{ConventionSource, Keymap, KeymapSource, Overrides, Platform, SourceError};

use super::super::{Action, build_keymap, declared, shortcut, terminal_shortcut};
use super::support::{GNOME, KDE, OTHER, OURS, Went, keymap_for, press, went};
use crate::error::SettingsError;

#[test]
fn every_declared_default_registers_except_where_a_standard_chord_has_it() {
    for platform in [Platform::MacOs, Platform::Windows, KDE, GNOME, OURS] {
        let (_, issues) = build_keymap(&ConventionSource, platform, &Overrides::default());
        assert!(issues.is_empty(), "{platform:?}: {issues:?}");
    }
    // Ctrl+Alt+Left and Right are the common conventions' Space switch.
    let (keymap, issues) = build_keymap(&ConventionSource, OTHER, &Overrides::default());
    let mentioned = |id: &str| {
        issues
            .iter()
            .filter(|(line, issue)| {
                *line == 0 && matches!(issue, SettingsError::Key(m) if m.contains(id))
            })
            .count()
    };
    assert_eq!(issues.len(), 2, "{issues:?}");
    assert_eq!(
        (
            mentioned("temor.focus-left"),
            mentioned("temor.focus-right")
        ),
        (1, 1)
    );
    // The refusal costs those two actions only.
    assert_eq!(
        went(&keymap, &press("Ctrl+Shift+Alt+Up")),
        Went::Ran(Action::FocusUp)
    );
}

#[test]
fn action_ids_are_the_ones_a_keys_file_writes() {
    #[rustfmt::skip]
    let rows: &[(Action, &str)] = &[
        (Action::Copy, "copy"),
        (Action::Paste, "paste"),
        (Action::Find, "find"),
        (Action::FindNext, "find-next"),
        (Action::FindPrevious, "find-previous"),
        (Action::NewTab, "temor.new-tab"),
        (Action::OpenBar, "temor.open-bar"),
        (Action::ToggleQuick, "temor.toggle-quick"),
        (Action::ClosePane, "close"),
        (Action::SplitRight, "temor.split-right"),
        (Action::SplitDown, "temor.split-down"),
        (Action::FocusLeft, "temor.focus-left"),
        (Action::FocusRight, "temor.focus-right"),
        (Action::FocusUp, "temor.focus-up"),
        (Action::FocusDown, "temor.focus-down"),
        (Action::PreviousPrompt, "temor.previous-prompt"),
        (Action::NextPrompt, "temor.next-prompt"),
        (Action::ToggleZoom, "temor.zoom"),
        (Action::Equalize, "temor.equalize"),
        (Action::ReopenTab, "temor.reopen-tab"),
        (Action::JumpToWaiting, "temor.jump-to-waiting"),
        (Action::ToggleSidebar, "temor.toggle-sidebar"),
        (Action::SelectTab(1), "temor.tab-1"),
        (Action::SelectTab(9), "temor.tab-9"),
        (Action::SelectSpace(3), "switch-to-space-3"),
    ];
    for (action, id) in rows {
        let named = action.to_chordkit().expect("bound by a keymap entry");
        assert_eq!(named.id(), *id);
        assert_eq!(Action::from_chordkit(&named), Some(*action), "{id}");
    }
    assert_eq!(Action::CloseFind.to_chordkit(), None);
    assert_eq!(Action::SelectTab(0).to_chordkit(), None);
    assert_eq!(Action::SelectTab(10).to_chordkit(), None);
    for id in [
        "select-all",
        "temor.nope",
        "temor.tab-0",
        "temor.tab-10",
        "mail.split-right",
    ] {
        let named: chordkit::Action = id.parse().expect("an action id");
        assert_eq!(Action::from_chordkit(&named), None, "{id}");
    }
}

#[test]
fn the_declared_defaults_are_temors_own_only() {
    let ids: Vec<String> = declared()
        .iter()
        .filter_map(|declared| declared.action.to_chordkit())
        .map(|named| named.id())
        .collect();
    assert_eq!(ids.len(), declared().len());
    assert!(ids.iter().all(|id| id.starts_with("temor.")), "{ids:?}");
}

#[test]
fn a_persons_changes_win_and_a_refused_one_keeps_the_default() {
    let (overrides, bad) =
        Overrides::parse("temor.split-right = Primary+Alt+N\ncopy = none\ntemor.zoom = Super+K\n");
    assert!(bad.is_empty(), "{bad:?}");
    let (keymap, issues) = build_keymap(&ConventionSource, Platform::Windows, &overrides);
    // Windows keeps every Windows-key chord for itself: line 3 is refused, in words.
    assert!(
        matches!(issues.as_slice(), [(3, SettingsError::Key(_))]),
        "{issues:?}"
    );
    use Went::{Dropped, Ran};
    for (text, want) in [
        ("Ctrl+Shift+Alt+N", Ran(Action::SplitRight)),
        ("Ctrl+Shift+D", Dropped),
        ("Ctrl+Shift+C", Dropped),
        ("Ctrl+Shift+Alt+Enter", Ran(Action::ToggleZoom)),
    ] {
        assert_eq!(went(&keymap, &press(text)), want, "{text}");
    }
}

/// A source whose settings cannot be read.
#[derive(Debug)]
struct Unreadable;

impl KeymapSource for Unreadable {
    fn load(&self, _platform: Platform) -> Result<Keymap, SourceError> {
        Err(SourceError::Unsupported("no shortcut settings here".into()))
    }
}

#[test]
fn an_unreadable_source_leaves_the_conventions() {
    let (keymap, issues) = build_keymap(&Unreadable, KDE, &Overrides::default());
    assert!(
        matches!(issues.as_slice(), [(0, SettingsError::Key(m))] if m.contains("no shortcut settings here")),
        "{issues:?}"
    );
    assert_eq!(
        went(&keymap, &press("Ctrl+Shift+C")),
        Went::Ran(Action::Copy)
    );
}

#[test]
fn shortcuts_are_shown_the_way_the_platform_writes_them() {
    let rows: &[(Platform, Action, Option<&str>, Option<&str>)] = &[
        (OURS, Action::Copy, Some("⌘C"), Some("⌘C")),
        (OURS, Action::SplitDown, Some("⇧⌘D"), Some("⇧⌘D")),
        (
            Platform::Windows,
            Action::SplitRight,
            Some("Ctrl+D"),
            Some("Ctrl+Shift+D"),
        ),
        (
            Platform::Windows,
            Action::SplitDown,
            Some("Ctrl+Shift+D"),
            Some("Ctrl+Alt+Shift+D"),
        ),
        (OURS, Action::SelectSpace(1), Some("⌃1"), None),
        (OURS, Action::CloseFind, None, None),
    ];
    for (platform, action, menu, terminal) in rows {
        let keymap = keymap_for(*platform);
        assert_eq!(
            shortcut(&keymap, *action).as_deref(),
            *menu,
            "{platform:?} {action:?}"
        );
        assert_eq!(
            terminal_shortcut(&keymap, *action).as_deref(),
            *terminal,
            "{platform:?} {action:?}"
        );
    }
}
