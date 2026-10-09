//! The single pane's pure parts (PLAN §6, §11 P1): key chords and the keymap, settings text,
//! selection, search, and paste rules. No I/O: the app edge reads files and drives the terminal.

pub mod action;
pub mod chord;
pub mod cwd;
pub mod keymap;
pub mod link;
pub mod paste;
pub mod row;
pub mod search;
pub mod selection;
pub mod settings;
pub mod settings_value;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod paste_tests;
#[cfg(test)]
mod selection_tests;
