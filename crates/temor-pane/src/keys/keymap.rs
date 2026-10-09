//! The pane's keymap: the default chords (PLAN §6) plus the person's overrides, checked so no
//! two actions share a chord and no chord is reserved for the desktop.

use super::action::Action;
use super::chord::{Chord, ChordKey, Mods, NamedKey};
use crate::error::SettingsError;

/// The actions and their chords. Looked up both ways; a chord belongs to one action at most.
/// A setting for an action replaces both of its default forms (see [`Keymap::bind`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<(Action, Chord)>,
}

/// A named key with modifiers, for a default chord.
fn ch_named(key: NamedKey, mods: Mods) -> Chord {
    Chord {
        key: ChordKey::Named(key),
        mods,
    }
}

/// Binds `action` to `key` with `mods`, in both ⌘ forms: Ctrl+Shift and Super.
fn command(bindings: &mut Vec<(Action, Chord)>, action: Action, key: ChordKey, mods: Mods) {
    for form in [Mods::CTRL.with(Mods::SHIFT), Mods::SUPER] {
        bindings.push((
            action,
            Chord {
                key,
                mods: mods.with(form),
            },
        ));
    }
}

impl Keymap {
    /// The defaults (PLAN §6). Every ⌘ action is bound twice: as Ctrl+Shift+key, the form keycap
    /// sends for ⌘ in a terminal window, and as Super+key, the form an unremapped ⌘ arrives in.
    pub fn defaults() -> Self {
        let mut bindings = Vec::new();

        let c = |ch: char| ChordKey::Char(ch);
        command(&mut bindings, Action::Copy, c('c'), Mods::NONE);
        command(&mut bindings, Action::Paste, c('v'), Mods::NONE);
        command(&mut bindings, Action::Find, c('f'), Mods::NONE);
        command(&mut bindings, Action::FindNext, c('g'), Mods::NONE);
        command(&mut bindings, Action::FindPrevious, c('g'), Mods::ALT);
        command(
            &mut bindings,
            Action::PreviousPrompt,
            ChordKey::Named(NamedKey::ArrowUp),
            Mods::NONE,
        );
        command(
            &mut bindings,
            Action::NextPrompt,
            ChordKey::Named(NamedKey::ArrowDown),
            Mods::NONE,
        );
        command(&mut bindings, Action::NewTab, c('t'), Mods::NONE);
        command(&mut bindings, Action::ClosePane, c('w'), Mods::NONE);
        command(&mut bindings, Action::SplitRight, c('d'), Mods::NONE);
        // ⌘⇧D would be Ctrl+Shift+Shift+D, the same chord as ⌘D: split down takes Alt in the
        // Ctrl+Shift form, and Shift in the Super form.
        bindings.push((
            Action::SplitDown,
            Chord::of_char('d', Mods::CTRL.with(Mods::SHIFT).with(Mods::ALT)),
        ));
        bindings.push((
            Action::SplitDown,
            Chord::of_char('d', Mods::SUPER.with(Mods::SHIFT)),
        ));
        let arrows = [
            (Action::FocusLeft, NamedKey::ArrowLeft),
            (Action::FocusRight, NamedKey::ArrowRight),
            (Action::FocusUp, NamedKey::ArrowUp),
            (Action::FocusDown, NamedKey::ArrowDown),
        ];
        for (action, key) in arrows {
            command(&mut bindings, action, ChordKey::Named(key), Mods::ALT);
        }
        command(
            &mut bindings,
            Action::ToggleZoom,
            ChordKey::Named(NamedKey::Enter),
            Mods::SHIFT,
        );
        // ⌃⌘=: the Super form is literal; the Ctrl+Shift form is what keycap sends for it.
        bindings.push((
            Action::Equalize,
            Chord::of_char('=', Mods::CTRL.with(Mods::SHIFT)),
        ));
        bindings.push((
            Action::Equalize,
            Chord::of_char('=', Mods::CTRL.with(Mods::SUPER)),
        ));
        // ⌘⇧T, like ⌘⇧D: Alt stands in for the second Shift in the Ctrl+Shift form.
        bindings.push((
            Action::ReopenTab,
            Chord::of_char('t', Mods::CTRL.with(Mods::SHIFT).with(Mods::ALT)),
        ));
        bindings.push((
            Action::ReopenTab,
            Chord::of_char('t', Mods::SUPER.with(Mods::SHIFT)),
        ));
        // ⌘1 to ⌘9 are Ctrl+Shift only: the Super digits belong to the shell (reserved).
        for n in 1..=9u8 {
            let digit = char::from(b'0' + n);
            bindings.push((
                Action::SelectTab(n),
                Chord::of_char(digit, Mods::CTRL.with(Mods::SHIFT)),
            ));
        }
        // The Space switch (`Action::SelectSpace`) has no default chord yet: quire leaves Ctrl+digit
        // to the desktop's workspace switch, and Super+digit belongs to the shell (PLAN §6). The
        // desktop rule decides which chord it gets; until then a person may bind one in settings.
        // Escape is the find bar's own key, not a ⌘ chord.
        bindings.push((Action::CloseFind, ch_named(NamedKey::Escape, Mods::NONE)));
        Self { bindings }
    }

    /// The action a chord runs, if any.
    pub fn action_for(&self, chord: &Chord) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(_, c)| c == chord)
            .map(|(a, _)| *a)
    }

    /// The first chord an action runs, if bound (the Ctrl+Shift form for a default ⌘ action).
    pub fn chord_for(&self, action: Action) -> Option<&Chord> {
        self.chords_for(action).next()
    }

    /// Every chord an action runs.
    pub fn chords_for(&self, action: Action) -> impl Iterator<Item = &Chord> {
        self.bindings
            .iter()
            .filter(move |(a, _)| *a == action)
            .map(|(_, c)| c)
    }

    /// Binds `action` to `chord` alone, replacing all its old chords: a setting for an action
    /// replaces both default forms, which is how a person gives a chord back to the program.
    /// Refused when the chord is reserved, when it is a bare Ctrl chord (those belong to the
    /// program), or when another action already uses it (binding an action to its own chord is
    /// allowed).
    pub fn bind(&mut self, action: Action, chord: Chord) -> Result<(), SettingsError> {
        if chord.is_reserved() {
            return Err(SettingsError::ReservedChord(chord.label()));
        }
        if let Some(refused) = chord.bare_ctrl_refusal() {
            return Err(refused);
        }
        if let Some((by, _)) = self
            .bindings
            .iter()
            .find(|(a, c)| *c == chord && *a != action)
        {
            return Err(SettingsError::ChordInUse {
                chord: chord.label(),
                by: by.label().to_owned(),
            });
        }
        self.bindings.retain(|(a, _)| *a != action);
        self.bindings.push((action, chord));
        Ok(())
    }
}
