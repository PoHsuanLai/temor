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
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/it");
    let main = std::fs::read_to_string(dir.join("main.rs")).expect("main.rs");
    for entry in std::fs::read_dir(&dir).expect("tests/it") {
        let path = entry.expect("entry").path();
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "main" {
            continue;
        }
        let is_module = path.is_dir() || path.extension().is_some_and(|e| e == "rs");
        if is_module {
            let decl = format!("mod {name};");
            assert!(
                main.contains(&decl),
                "tests/it/{name} is not declared in main.rs"
            );
        }
    }
}
