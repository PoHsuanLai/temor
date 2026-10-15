//! temor-transfer's tests: parsers, the receive machine, traversal refusal, the shelf and fuzzing.
//! Filesystem tests use temporary directories only.

mod fuzz;
#[cfg(target_os = "linux")]
mod kitty_interop;
mod kitty_receive;
mod kitty_receive_limits;
mod kitty_receive_names;
mod kitty_send;
mod kitty_wire;
#[cfg(target_os = "linux")]
mod kitty_writes;
mod osc1337;
mod paths;
mod receive_support;
#[cfg(target_os = "linux")]
mod roundtrip;
mod shelf;
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
