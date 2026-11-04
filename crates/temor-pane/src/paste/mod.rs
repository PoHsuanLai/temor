//! Paste rules (PLAN §6): what makes pasted text need the person's confirmation, and how the
//! bytes are framed for the program.

mod rules;
#[cfg(test)]
mod tests;

pub use rules::{PasteCheck, PasteMode, UnsafeReason, check, encode};
