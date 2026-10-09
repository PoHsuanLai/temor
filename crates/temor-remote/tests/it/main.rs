//! temor-remote's tests: an in-process daemon and client over pipes, real terminals running the
//! test shells, a scratch HOME and scratch folders (never the real ones). Where bash is missing
//! the terminal tests skip, and fail under TEMOR_REQUIRE_TOOLS=1.

#[cfg(feature = "ghostty")]
mod files;
#[cfg(feature = "ghostty")]
mod handshake;
#[cfg(feature = "ghostty")]
mod ports;
#[cfg(feature = "ghostty")]
mod sessions;
#[cfg(feature = "ghostty")]
mod support;
#[cfg(feature = "ghostty")]
mod transfer;

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
