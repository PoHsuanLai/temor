//! Who keeps an agent's file history (PLAN P19): docket saves a restore point before each turn,
//! except for agents that keep their own.

mod table;
#[cfg(test)]
mod tests;

pub use table::{Rewind, rewind_of};
