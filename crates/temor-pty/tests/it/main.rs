//! temor-pty's integration tests: real terminals, real shells, scratch HOME and runtime
//! directories (never the real ones).

// Only Linux has a real terminal yet; elsewhere `Pty::spawn` is `Unsupported` (see the
// crate's `platform` module), so there is nothing here to run.
#![cfg(target_os = "linux")]

mod dotfiles;
mod env_contract;
mod foreground;
mod integration;
mod kill;
mod opt_out;
mod resize;
mod support;
