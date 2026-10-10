//! temor-vt conformance: recorded byte streams in `tests/fixtures/*.bin`, each module pairing its
//! fixtures with the outcomes libghostty must produce. A Ghostty pin bump must pass these.

mod fake;

#[cfg(feature = "ghostty")]
mod support;

#[cfg(feature = "ghostty")]
mod anchors;
#[cfg(feature = "ghostty")]
mod cells;
mod colors;
#[cfg(feature = "ghostty")]
mod graphemes;
#[cfg(feature = "ghostty")]
mod holds;
#[cfg(feature = "ghostty")]
mod inline_file;
#[cfg(feature = "ghostty")]
mod keys;
#[cfg(feature = "ghostty")]
mod kitty;
#[cfg(feature = "ghostty")]
mod mouse;
#[cfg(feature = "ghostty")]
mod replies;
#[cfg(feature = "ghostty")]
mod screen;
#[cfg(feature = "ghostty")]
mod sgr;
#[cfg(feature = "ghostty")]
mod shell;
#[cfg(feature = "ghostty")]
mod snapshot;
#[cfg(feature = "ghostty")]
mod threads;

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
