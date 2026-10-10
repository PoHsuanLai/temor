//! temor-pane's engine tests: the pane on a real PTY and bash (scratch HOME), driven by direct
//! calls. Where bash, setsid or /proc is missing the tests skip (a failure under
//! TEMOR_REQUIRE_TOOLS=1).

#[cfg(feature = "ghostty")]
mod clipboard;
#[cfg(feature = "ghostty")]
mod engine;
#[cfg(feature = "ghostty")]
mod holds;
#[cfg(feature = "ghostty")]
mod support;
#[cfg(feature = "ghostty")]
mod transfers;

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
