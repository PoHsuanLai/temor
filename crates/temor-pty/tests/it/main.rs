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
mod nushell_env;
mod nushell_offer;
mod opt_out;
mod prompt;
mod resize;
mod support;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}
