//! `FakeVt`: a small terminal in plain Rust for tests that must not need Zig. It understands
//! printable text, CR/LF/BS/TAB, BEL, cursor position, erase display, DEC private modes, DA1 and
//! OSC 0/2/7/133, and runs the same inline-file pre-scan as the real terminal. Everything else is
//! swallowed (unknown OSCs are reported as [`crate::UnknownKind::Osc`]).

mod parse;
mod screen;
mod terminal;
#[cfg(test)]
mod tests;

pub use terminal::FakeVt;
