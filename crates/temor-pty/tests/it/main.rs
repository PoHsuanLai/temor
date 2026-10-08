//! temor-pty's integration tests: real terminals, real shells, scratch HOME and runtime
//! directories (never the real ones). Where the platform has no terminal yet, each test
//! prints a skip note and passes.

mod dotfiles;
mod env_contract;
mod fish_env;
mod foreground;
mod integration;
mod kill;
mod line_discipline;
mod opt_out;
mod resize;
mod support;
