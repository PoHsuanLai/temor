//! temor-core's one integration-test binary: the public surface, through property tests and
//! round trips.

mod bar_rank;
mod sanitise_props;
mod serde_shapes;
mod split_props;
mod worktree_git;

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
