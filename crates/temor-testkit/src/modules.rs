//! The check that every file in a crate's `tests/it/` is declared in its `main.rs`.

use std::path::Path;

/// Panics when a `.rs` file or folder next to `tests/it/main.rs` is not declared there as
/// `mod <name>;`. Call it from a test with `env!("CARGO_MANIFEST_DIR")`.
pub fn assert_modules_declared(manifest_dir: &str) {
    let dir = Path::new(manifest_dir).join("tests/it");
    let main = std::fs::read_to_string(dir.join("main.rs")).expect("tests/it/main.rs");
    for entry in std::fs::read_dir(&dir).expect("tests/it") {
        let path = entry.expect("a directory entry").path();
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if name == "main" {
            continue;
        }
        let is_module = path.is_dir() || path.extension().is_some_and(|ext| ext == "rs");
        if is_module {
            let declaration = format!("mod {name};");
            assert!(
                main.contains(&declaration),
                "tests/it/{name} is not declared in main.rs"
            );
        }
    }
}
