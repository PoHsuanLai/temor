//! The pane's keymap: the default chords (PLAN §6) plus the person's overrides, checked so no
//! two actions share a chord and no chord is reserved for the desktop.

use super::action::Action;
use super::chord::{Chord, ChordKey, Mods, NamedKey};
use crate::error::CoreError;
use ds_core::word::Word;

/// Each action with its one chord. Looked up both ways, so a binding is never ambiguous.
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

impl Keymap {
    /// The defaults. Every chord is the Ctrl+Shift form of the ⌘ chord the person sees.
    pub fn defaults() -> Self {
        let ctrl_shift = Mods::CTRL.with(Mods::SHIFT);
        let ch = |c: char, mods: Mods| Chord::of_char(c, mods);
        Self {
            bindings: vec![
                (Action::Copy, ch('c', ctrl_shift)),
                (Action::Paste, ch('v', ctrl_shift)),
                (Action::Find, ch('f', ctrl_shift)),
                (Action::FindNext, ch('g', ctrl_shift)),
                (Action::FindPrevious, ch('g', ctrl_shift.with(Mods::ALT))),
                (
                    Action::PreviousPrompt,
                    ch_named(NamedKey::ArrowUp, ctrl_shift),
                ),
                (
                    Action::NextPrompt,
                    ch_named(NamedKey::ArrowDown, ctrl_shift),
                ),
                (
                    Action::CloseFind,
                    Chord {
                        key: ChordKey::Named(NamedKey::Escape),
                        mods: Mods::NONE,
                    },
                ),
            ],
        }
    }

    /// The action a chord runs, if any.
    pub fn action_for(&self, chord: &Chord) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(_, c)| c == chord)
            .map(|(a, _)| *a)
    }

    /// The chord an action runs, if bound.
    pub fn chord_for(&self, action: Action) -> Option<&Chord> {
        self.bindings
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, c)| c)
    }

    /// Binds `action` to `chord`, replacing its old chord. Refused when the chord is reserved
    /// or another action already uses it (binding an action to its own chord is allowed).
    pub fn bind(&mut self, action: Action, chord: Chord) -> Result<(), CoreError> {
        if chord.is_reserved() {
            return Err(CoreError::ReservedChord(chord.label()));
        }
        if let Some((by, _)) = self
            .bindings
            .iter()
            .find(|(a, c)| *c == chord && *a != action)
        {
            return Err(CoreError::ChordInUse {
                chord: chord.label(),
                by: by.label().to_owned(),
            });
        }
        match self.bindings.iter_mut().find(|(a, _)| *a == action) {
            Some(slot) => slot.1 = chord,
            None => self.bindings.push((action, chord)),
        }
        Ok(())
    }
}
