//! Key chords, the pane's actions, and the keymap that binds actions to chords (PLAN §6).

mod action;
mod chord;
mod keymap;
#[cfg(test)]
mod tests;

pub use action::Action;
pub use chord::{Chord, ChordKey, Mods, NamedKey, fold};
pub use keymap::Keymap;
