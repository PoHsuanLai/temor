//! temor's key actions and how they meet chordkit's keymap (PLAN §6). temor owns no chords,
//! modifiers or Command-versus-Ctrl rule: it names its actions, declares their `Primary+…`
//! defaults, and asks the keymap what a key means in a terminal.

mod action;
mod build;
mod declare;
mod edit;
#[cfg(test)]
mod tests;

pub use action::Action;
pub use build::{KeyIssue, build_keymap, shortcut, terminal_shortcut};
pub use declare::{Declared, declared};
pub use edit::{bind_key, is_key_action, reset_all_keys, reset_key};
